// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

#[cfg(target_os = "windows")]
fn main() {
    eprintln!(
        "guestkit-qemu is a host-side QEMU tool and is not built for the Windows guest agent"
    );
    std::process::exit(2);
}

#[cfg(not(target_os = "windows"))]
fn main() -> anyhow::Result<()> {
    host::run()
}

#[cfg(not(target_os = "windows"))]
mod host {
    use anyhow::{bail, Context};
    use clap::{Args, Parser, Subcommand, ValueEnum};
    use guestkit::assurance::collect_assurance_data;
    use guestkit::boot::BootTarget;
    #[cfg(unix)]
    use guestkit::qemu::qmp::QmpClient;
    use guestkit::qemu::{
        Acceleration, CacheMode, Console, DiskFormat, DiskInterface, Firmware, ForwardProtocol,
        GuestKitQemuOptions, GuestKitQemuPlan, HostForward, NetworkBackend, NetworkModel,
    };
    use std::path::PathBuf;

    #[derive(Parser)]
    #[command(name = "guestkit-qemu")]
    #[command(version = guestkit::VERSION)]
    #[command(about = "GuestKit-assured QEMU/VirtIO VM planner and launcher")]
    struct Cli {
        #[command(subcommand)]
        command: Command,
    }

    #[derive(Subcommand)]
    enum Command {
        /// Inspect the image with GuestKit and render the derived QEMU configuration.
        Plan(VmArgs),

        /// Run a VM after GuestKit boot-assurance gating.
        Run {
            #[command(flatten)]
            vm: VmArgs,

            /// Minimum GuestKit boot assurance score required for launch.
            #[arg(long, default_value_t = 70.0)]
            min_boot_score: f64,

            /// Start even when GuestKit reports blockers, a low score, or missing UEFI firmware.
            #[arg(long)]
            allow_unready: bool,
        },

        /// Send a day-2 command to a QEMU Machine Protocol Unix socket.
        #[cfg(unix)]
        Qmp {
            #[arg(long, value_name = "PATH")]
            socket: PathBuf,

            #[command(subcommand)]
            action: QmpAction,
        },
    }

    #[cfg(unix)]
    #[derive(Subcommand)]
    enum QmpAction {
        Status,
        Pause,
        Resume,
        Powerdown,
        Quit,
        Balloon {
            /// Requested guest memory size in MiB.
            mib: u64,
        },
    }

    #[derive(Args, Clone)]
    struct VmArgs {
        /// Guest disk image. GuestKit inspects this before QEMU planning.
        image: PathBuf,

        #[arg(long)]
        name: Option<String>,

        #[arg(long, default_value_t = 4096)]
        memory_mb: u64,

        #[arg(long, default_value_t = 2)]
        vcpus: u16,

        /// Use TCG software emulation instead of KVM.
        #[arg(long)]
        tcg: bool,

        /// Override disk format instead of inferring it from the image suffix.
        #[arg(long, value_enum)]
        disk_format: Option<DiskFormatArg>,

        #[arg(long, value_enum, default_value = "virtio-blk")]
        disk_bus: DiskBusArg,

        #[arg(long, value_enum, default_value = "none")]
        cache: CacheArg,

        /// Disable discard/unmap advertisement to the guest.
        #[arg(long)]
        no_discard: bool,

        /// Use an existing host TAP interface instead of user-mode networking.
        #[arg(long, conflicts_with = "bridge")]
        tap: Option<String>,

        /// Attach the NIC to a QEMU bridge backend instead of user-mode networking.
        #[arg(long, conflicts_with = "tap")]
        bridge: Option<String>,

        /// Enable vhost acceleration for --tap networking.
        #[arg(long, requires = "tap")]
        vhost: bool,

        #[arg(long)]
        mac: Option<String>,

        /// Forward host TCP port to guest SSH port 22 (user-mode networking only).
        #[arg(long, conflicts_with_all = ["tap", "bridge"])]
        ssh_forward: Option<u16>,

        /// UEFI firmware code image (OVMF_CODE/AAVMF_CODE).
        #[arg(long, value_name = "FILE")]
        uefi_code: Option<PathBuf>,

        /// Writable UEFI vars image paired with --uefi-code.
        #[arg(long, value_name = "FILE", requires = "uefi_code")]
        uefi_vars: Option<PathBuf>,

        #[arg(long, value_enum, default_value = "serial")]
        console: ConsoleArg,

        /// VNC display number used when --console=vnc.
        #[arg(long, default_value_t = 0)]
        vnc_display: u16,

        /// SPICE TCP port used when --console=spice.
        #[arg(long, default_value_t = 5901)]
        spice_port: u16,

        /// Create a QMP Unix socket for runtime control.
        #[arg(long, value_name = "PATH")]
        qmp_socket: Option<PathBuf>,

        /// Add a vhost-vsock device with this guest CID (must be >= 3).
        #[arg(long)]
        vsock_cid: Option<u32>,

        /// QEMU executable override; useful for pinned/custom builds.
        #[arg(long, value_name = "PATH")]
        qemu_binary: Option<PathBuf>,

        /// Write QEMU PID to this file.
        #[arg(long, value_name = "PATH")]
        pidfile: Option<PathBuf>,

        /// Ask QEMU to daemonize. Use --console=none/vnc/spice.
        #[arg(long)]
        daemonize: bool,

        /// Print the complete plan as JSON.
        #[arg(long)]
        json: bool,

        /// Show GuestKit inspection progress.
        #[arg(short, long)]
        verbose: bool,
    }

    #[derive(Clone, Copy, Debug, ValueEnum)]
    enum DiskFormatArg {
        Raw,
        Qcow2,
        Vmdk,
        Vdi,
        Vhd,
    }

    impl From<DiskFormatArg> for DiskFormat {
        fn from(value: DiskFormatArg) -> Self {
            match value {
                DiskFormatArg::Raw => Self::Raw,
                DiskFormatArg::Qcow2 => Self::Qcow2,
                DiskFormatArg::Vmdk => Self::Vmdk,
                DiskFormatArg::Vdi => Self::Vdi,
                DiskFormatArg::Vhd => Self::Vhd,
            }
        }
    }

    #[derive(Clone, Copy, Debug, ValueEnum)]
    enum DiskBusArg {
        VirtioBlk,
        VirtioScsi,
        Nvme,
        Sata,
    }

    impl From<DiskBusArg> for DiskInterface {
        fn from(value: DiskBusArg) -> Self {
            match value {
                DiskBusArg::VirtioBlk => Self::VirtioBlk,
                DiskBusArg::VirtioScsi => Self::VirtioScsi,
                DiskBusArg::Nvme => Self::Nvme,
                DiskBusArg::Sata => Self::Sata,
            }
        }
    }

    #[derive(Clone, Copy, Debug, ValueEnum)]
    enum CacheArg {
        None,
        Writeback,
        Writethrough,
        Directsync,
        Unsafe,
    }

    impl From<CacheArg> for CacheMode {
        fn from(value: CacheArg) -> Self {
            match value {
                CacheArg::None => Self::None,
                CacheArg::Writeback => Self::Writeback,
                CacheArg::Writethrough => Self::Writethrough,
                CacheArg::Directsync => Self::Directsync,
                CacheArg::Unsafe => Self::Unsafe,
            }
        }
    }

    #[derive(Clone, Copy, Debug, ValueEnum)]
    enum ConsoleArg {
        Serial,
        None,
        Vnc,
        Spice,
    }

    pub fn run() -> anyhow::Result<()> {
        let cli = Cli::parse();
        match cli.command {
            Command::Plan(args) => {
                let plan = build_plan(&args)?;
                print_plan(&plan, args.json)?;
            }
            Command::Run {
                vm,
                min_boot_score,
                allow_unready,
            } => {
                if !(0.0..=100.0).contains(&min_boot_score) {
                    bail!("--min-boot-score must be between 0 and 100");
                }
                let plan = build_plan(&vm)?;
                print_plan(&plan, vm.json)?;
                if !allow_unready {
                    plan.enforce_ready(min_boot_score)
                        .context("GuestKit QEMU launch gate failed")?;
                }
                let command = plan.vm.command_spec()?;
                eprintln!("Starting: {}", command.render_shell());
                let mut child = plan.vm.spawn()?;
                let status = child.wait().context("failed waiting for QEMU")?;
                if !status.success() {
                    bail!("QEMU exited with status {status}");
                }
            }
            #[cfg(unix)]
            Command::Qmp { socket, action } => run_qmp(socket, action)?,
        }
        Ok(())
    }

    fn build_plan(args: &VmArgs) -> anyhow::Result<GuestKitQemuPlan> {
        let (evidence, boot) =
            collect_assurance_data(&args.image, BootTarget::Kvm, args.verbose)
                .with_context(|| format!("GuestKit could not inspect {}", args.image.display()))?;

        let network_backend = if let Some(tap) = &args.tap {
            NetworkBackend::Tap {
                interface: tap.clone(),
                vhost: args.vhost,
            }
        } else if let Some(bridge) = &args.bridge {
            NetworkBackend::Bridge {
                bridge: bridge.clone(),
            }
        } else {
            let forwards = args
                .ssh_forward
                .map(|host_port| {
                    vec![HostForward {
                        protocol: ForwardProtocol::Tcp,
                        host_addr: Some("127.0.0.1".into()),
                        host_port,
                        guest_addr: None,
                        guest_port: 22,
                    }]
                })
                .unwrap_or_default();
            NetworkBackend::User { forwards }
        };

        let firmware = args.uefi_code.as_ref().map(|code| Firmware {
            code: code.clone(),
            vars: args.uefi_vars.clone(),
        });
        let console = match args.console {
            ConsoleArg::Serial => Console::Serial,
            ConsoleArg::None => Console::None,
            ConsoleArg::Vnc => Console::Vnc {
                display: args.vnc_display,
            },
            ConsoleArg::Spice => Console::Spice {
                port: args.spice_port,
            },
        };

        GuestKitQemuPlan::from_assurance(
            &args.image,
            &evidence,
            &boot,
            GuestKitQemuOptions {
                name: args.name.clone(),
                memory_mb: args.memory_mb,
                vcpus: args.vcpus,
                acceleration: if args.tcg {
                    Acceleration::Tcg
                } else {
                    Acceleration::Kvm
                },
                disk_format: args.disk_format.map(Into::into),
                disk_interface: args.disk_bus.into(),
                cache: args.cache.into(),
                discard: !args.no_discard,
                network_backend,
                network_model: NetworkModel::VirtioNet,
                mac: args.mac.clone(),
                firmware,
                console,
                qmp_socket: args.qmp_socket.clone(),
                vsock_cid: args.vsock_cid,
                daemonize: args.daemonize,
                pidfile: args.pidfile.clone(),
                binary_override: args.qemu_binary.clone(),
            },
        )
        .context("could not derive QEMU configuration from GuestKit evidence")
    }

    fn print_plan(plan: &GuestKitQemuPlan, json: bool) -> anyhow::Result<()> {
        if json {
            println!("{}", serde_json::to_string_pretty(plan)?);
            return Ok(());
        }

        println!("GuestKit QEMU plan");
        println!("  OS:          {}", plan.guest_os);
        println!("  Architecture: {:?}", plan.vm.architecture);
        println!("  Boot score:  {:.0}/100", plan.boot_score);
        println!("  Confidence:  {:.0}%", plan.boot_confidence * 100.0);
        println!("  Blockers:    {}", plan.blockers.len());
        for blocker in &plan.blockers {
            println!("    BLOCKER: {blocker}");
        }
        for warning in &plan.warnings {
            println!("    WARNING: {warning}");
        }
        println!("  Command:");
        println!("    {}", plan.vm.command_spec()?.render_shell());
        Ok(())
    }

    #[cfg(unix)]
    fn run_qmp(socket: PathBuf, action: QmpAction) -> anyhow::Result<()> {
        let mut qmp = QmpClient::connect(&socket)
            .with_context(|| format!("could not connect to QMP socket {}", socket.display()))?;
        match action {
            QmpAction::Status => println!("{}", qmp.query_status()?),
            QmpAction::Pause => qmp.pause()?,
            QmpAction::Resume => qmp.resume()?,
            QmpAction::Powerdown => qmp.powerdown()?,
            QmpAction::Quit => qmp.quit()?,
            QmpAction::Balloon { mib } => qmp.balloon(
                mib.checked_mul(1024 * 1024)
                    .context("balloon size overflows u64")?,
            )?,
        }
        Ok(())
    }
}
