# GuestKit QEMU / VirtIO runtime

GuestKit's QEMU runtime turns migration evidence into an executable VM definition.
The design keeps **inspection and assurance** separate from **runtime execution**:

1. GuestKit mounts the image read-only and produces `EvidenceSnapshot`.
2. The existing KVM bootability engine scores the image and returns blockers/warnings.
3. `GuestKitQemuPlan` derives architecture, machine type, disk format, VirtIO devices,
   networking, firmware requirements, and a safe QEMU argv vector.
4. `guestkit-qemu run` enforces the assurance gate before spawning QEMU unless
   `--allow-unready` is explicitly supplied.
5. QMP provides pause/resume/status/powerdown/balloon/quit day-2 operations.

## Plan a VM

```bash
guestkit-qemu plan /var/lib/vms/app01.qcow2 \
  --memory-mb 8192 \
  --vcpus 4 \
  --qmp-socket /run/guestkit/app01.qmp
```

For UEFI guests, pass the host firmware explicitly instead of GuestKit guessing a
distribution-specific path:

```bash
guestkit-qemu plan /var/lib/vms/app01.qcow2 \
  --uefi-code /usr/share/OVMF/OVMF_CODE.fd \
  --uefi-vars /var/lib/vms/app01_VARS.fd
```

## Run with a migration-assurance gate

```bash
guestkit-qemu run /var/lib/vms/app01.qcow2 \
  --memory-mb 8192 \
  --vcpus 4 \
  --min-boot-score 80 \
  --qmp-socket /run/guestkit/app01.qmp
```

The launcher refuses to start by default when GuestKit reports a boot blocker, the
score is below the requested threshold, or a UEFI guest has no pflash firmware.
`--allow-unready` is an explicit operator override.

## Networking

User-mode networking is the safe default. SSH forwarding binds to loopback only:

```bash
guestkit-qemu run app01.qcow2 --ssh-forward 2222
```

For production networking, attach an already-provisioned TAP or QEMU bridge:

```bash
guestkit-qemu run app01.qcow2 --tap tap-app01 --vhost
# or
guestkit-qemu run app01.qcow2 --bridge br0
```

GuestKit does not create TAP devices, bridges, firewall rules, or host routes in
this module. Those remain orchestration responsibilities — for production run +
network + TTL, use **[FluxVM](https://github.com/zyvorai/fluxvm)** (see
[vm-runtime.md](vm-runtime.md) suite goal).

## QMP day-2 control

```bash
guestkit-qemu qmp --socket /run/guestkit/app01.qmp status
guestkit-qemu qmp --socket /run/guestkit/app01.qmp pause
guestkit-qemu qmp --socket /run/guestkit/app01.qmp resume
guestkit-qemu qmp --socket /run/guestkit/app01.qmp balloon 4096
guestkit-qemu qmp --socket /run/guestkit/app01.qmp powerdown
```

The QMP client negotiates `qmp_capabilities`, ignores asynchronous events while
waiting for command replies, and returns QMP errors instead of treating them as
successful commands.

## Library usage

```rust,no_run
use guestkit::qemu::{
    Architecture, CacheMode, Disk, DiskFormat, DiskInterface, QemuVm,
};

let vm = QemuVm::new("app01", Architecture::X86_64).disk(Disk {
    id: "root".into(),
    path: "/var/lib/vms/app01.qcow2".into(),
    format: DiskFormat::Qcow2,
    interface: DiskInterface::VirtioBlk,
    readonly: false,
    cache: CacheMode::None,
    discard: true,
});

println!("{}", vm.command_spec()?.render_shell());
# Ok::<(), guestkit::qemu::QemuError>(())
```

Runtime execution uses `std::process::Command` with an argument vector. The shell
string is only for logs/copy-paste and is never used to launch the process.

## Capacity matrix

| Area | Supported |
|------|-----------|
| Arch | x86_64, AArch64 |
| Machines | Q35, PC, ARM `virt` |
| Accel | KVM, TCG (`-cpu host` rejected under TCG) |
| Disk | VirtIO-blk, VirtIO-SCSI (shared controller), NVMe, SATA |
| Formats | QCOW2, RAW, VMDK, VDI, VHD |
| Net | user (SSH → `127.0.0.1`), TAP (`vhost=on`), bridge |
| Devices | balloon, RNG, vsock, VirtIO GPU, serial |
| Firmware | OVMF / AAVMF pflash |
| Console | serial, VNC, SPICE |
| Control | QMP Unix socket, PID file / daemonize |

## See also

- [Migration assurance](migration-assurance.md)
- [h2kvm integration](hyper2kvm-integration.md)
- [CLI quick reference](../user-guides/quick-reference.md)
- [Architecture overview](../architecture/overview.md)
