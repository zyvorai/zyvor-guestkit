# CLI reference (v1.2.11)

`guestkit` and `guestctl` are the same binary and share one command surface (`guestctl` is a symlink to `guestkit` in the release tarball). The separate
**`guestkit-qemu`** binary plans and launches QEMU from the same assurance
engine. Use **`guestkit --help`**, **`guestkit <command> --help`**, and
**`guestkit-qemu --help`** for the authoritative flag list; this page summarises
the command surface and links the curated docs.

Running `guestkit <disk-image>` with a single disk path (for example `guestkit vm.qcow2`) is shorthand for `guestkit inspect vm.qcow2`.

## Quick start

```bash
guestkit inspect disk.qcow2
guestkit doctor disk.qcow2 --target proxmox --explain
guestkit doctor disk.qcow2 --target proxmox -o json --fail-below 80
guestkit migrate-plan disk.qcow2 --target kvm --export plan.yaml
guestctl tui disk.qcow2
guestkit-qemu plan disk.qcow2 --json
guestkit-qemu run disk.qcow2 --min-boot-score 80
guestkit qga --execute guest-ping   # needs a build with --features agent
```

`guestkit-qemu` and `virtctl-guestkit` are **not** in the release tarball (it ships `guestkit` and the `guestctl` symlink only). Build them from source with `cargo build --release`; see [Prerequisites and build](../prerequisites-and-build.md).

## Where to look

| Topic | Doc |
|-------|-----|
| Cheat sheet | [quick-reference.md](quick-reference.md) |
| Install & build | [getting-started.md](getting-started.md) |
| Migration assurance | [migration-assurance.md](../features/migration-assurance.md) |
| QEMU / VirtIO runtime | [qemu-runtime.md](../features/qemu-runtime.md) |
| Dump virsh → GuestKit | [virsh-to-guestkit.md](virsh-to-guestkit.md) |
| Guest agent | [guest-agent.md](../features/guest-agent.md) |
| VM migration workflows | [vm-migration.md](vm-migration.md) |
| TUI keys & Assurance | [tui-enhancements.md](../features/tui-enhancements.md) |
| Fix plans & rescue | [fix-plans.md](../features/fix-plans.md) |
| Profiles | [profiles.md](profiles.md) |
| Interactive REPL | [interactive-mode.md](interactive-mode.md) |
| File explorer | [EXPLORE-QUICKSTART.md](../features/explore/EXPLORE-QUICKSTART.md) |
| Cloud disk sources | [cloud-disk-sources.md](../guides/cloud-disk-sources.md) |
| Python API | [python-bindings.md](python-bindings.md) |
| Serial console without grubby | [img-firstboot.md](img-firstboot.md#serial-console-without-grubby) |
| FAQ | [faq.md](faq.md) |
| Troubleshooting | [troubleshooting.md](troubleshooting.md) |

## Global flags

These are accepted by every subcommand.

| Flag | Meaning |
|------|---------|
| `--verbose` | Verbose output (debug log level) |
| `--debug` | Show internal operations (sets `GUESTKIT_DEBUG=1`) |
| `-q`, `--quiet` | Suppress non-error output (conflicts with `--verbose`) |
| `--no-color` | Disable colored output |
| `--timestamps` | Timestamps in log output |
| `--machine-readable` | Implies `--no-color` |
| `-R`, `--read-only` | Documented as "prevent write operations"; exports `GUESTKIT_READONLY=1` |
| `-T`, `--timeout SECONDS` | Documented as an operation timeout (`0` = none); exports `GUESTKIT_TIMEOUT` |
| `--cache-dir DIR` | Documented as a custom cache directory; exports `GUESTKIT_CACHE_DIR` |
| `-j`, `--jobs N` | Documented as parallel workers |

**Caveat from reading the v1.2.11 source:** `--read-only`, `--timeout` and `--cache-dir` only set the environment variables above, and nothing in the source reads `GUESTKIT_READONLY`, `GUESTKIT_TIMEOUT` or `GUESTKIT_CACHE_DIR`; the global `--jobs` value is not read either. This was established by searching the code, not by running the binary. Do not rely on `--read-only` to protect an image: use read-only commands, `guestkit doctor`/`inspect`, or a copy of the disk. The per-command parallelism flags (`fleet ... -j`, `inspect-batch --parallel`) are separate per-command arguments. The inspection cache is `~/.cache/guestkit/`.

## Command groups

`guestkit commands` prints the grouped list that ships in your binary. The table below is checked against the clap definitions in `src/cli/entry.rs` for v1.2.11; aliases are in parentheses.

| Group | Commands |
|-------|----------|
| Inspect | `inspect`, `inspect-batch`, `detect`, `info`, `filesystems` (`fs`), `packages` (`pkg`), `network`, `inventory`, `scan`, `diff`, `compare` |
| Files | `list` (`ls`), `extract` (`get`), `cat`, `search` (`find`), `grep`, `hash`, `copy`, `backup`, `find-large`, `find-duplicates`, `diff-files`, `disk-usage`, `usage` (`df`), `check` (`fsck`) |
| Disk images | `convert`, `create`, `shrink`, `img` (`info`, `check`, `snapshots`, `snapshot-create`, `snapshot-delete`, `snapshot-apply`, `resize`, `rebase`, `commit`), `snapshot`, `clone`, `optimize`, `cache-clear`, `cache-stats` |
| Assurance | `doctor`, `migrate-plan`, `migrate-assess`, `migrate-repair`, `policy` (`check`, `rego`), `cloud-profile`, `fleet` (`analyze`, `wave-plan`, `watch`, `quarantine`), `forensic-diff`, `sbom-diff`, `gate`, `passport` (`emit`, `verify`, `keygen`, `handoff`) |
| Repair and rescue | `repair`, `rescue`, `plan`, `harden`, `patch`, `sysprep`, `selinux-relabel`, `virtio-initramfs`, `firstboot`, `cloud-init`, `bitlocker` (`status`, `escrow`), `virtio-win` (`list`, `plan`) |
| Security and compliance | `secrets`, `compliance`, `malware`, `health`, `audit`, `anomaly`, `recommend`, `predict`, `hunt`, `reconstruct`, `evolve`, `verify` |
| Live guest and runtime | `qga`, `agent`, `agent-proxy`, `agent-call`, `agent-inject`, `agent-sign`, `vm`, `domain-disks`, `execute` (`exec`) |
| Systemd | `systemd-journal`, `systemd-services`, `systemd-boot` |
| Interactive | `tui` (`ui`), `shell` (`sh`), `interactive` (`repl`), `explore` (`ex`), `script` (`batch`), `ai` |
| Analysis and planning | `analyze`, `benchmark`, `timeline`, `fingerprint`, `drift`, `validate`, `license`, `blueprint`, `migrate`, `cost`, `dependencies`, `intelligence`, `simulate`, `score`, `template` |
| Utilities | `completion` (bash, zsh, fish, powershell, elvish), `commands`, `version`, `mcp-serve` (needs the `mcp` feature) |

There is no `cp`, `download`, `upload` or top-level `profile` command. Use `copy` (between two images), `extract` (guest to host) and `plan`/`rescue`/`migrate-repair` for writes. Inspection profiles are selected with `inspect --profile` (`security`, `migration`, `performance`, `compliance`, `hardening`, `windows-migration`).

### Targets

`doctor`, `migrate-plan`, `migrate-assess`, `migrate-repair`, `passport emit` and `gate` accept `kvm`, `qemu`, `kubevirt`, `proxmox`, `hyperv`, `cloud`, `aws`, `azure` and `gcp`. `aws`, `azure` and `gcp` score as `cloud`. An unknown name is not rejected: the boot engine falls back to a generic profile. `--help` for `doctor` lists only `kvm, proxmox, hyperv, cloud`.

### Frequently used flags

| Command | Flags |
|---------|-------|
| `inspect` | `-o, --output text\|json\|yaml\|csv`, `-p, --profile`, `-e, --export html\|markdown\|pdf`, `--export-output PATH`, `--no-cache`, `--cache-refresh`, `-S, --summary`, `--include-packages`, `--include-services`, `--include-network`, `--depth quick\|standard\|deep`, `--save-report FILE` |
| `doctor` | `--target` (default `kvm`), `--explain`, `--ai` (needs the `ai` feature), `-o text\|json`, `--fail-below SCORE` (exit 1 under the score) |
| `migrate-plan` | `--target` (required), `--explain`, `--ai`, `-o text\|json`, `--export FILE` (YAML or JSON by extension), `--inject-agent`, `--agent-binary`, `--agent-unit` |
| `migrate-assess` | `--target`, `-o`, `--fail-below SCORE` |
| `migrate-repair` | `--target`, `--apply` (takes a full-image backup first), `--include-destructive`, `--virtio-win DIR`, `--export FILE` |
| `repair` | `--fix boot` or `-t, --repair-type permissions\|packages\|network\|bootloader\|filesystem\|boot` (one is required), `--dry-run`, `-f`, `-b`, `--inject-agent`, `--agent-binary`, `--agent-unit` |
| `rescue` | `-o, --operation` one of `reset-password`, `fix-fstab`, `check-grub`, `fix-grub`, `enable-ssh`, `inject-ssh-key`, `set-hostname`, `enable-rdp`, `enable-winrm`, `set-timezone`, `install-packages`; plus `-u`, `-p`, `--key`, `--key-file`, `--hostname`, `--timezone`, `--packages a,b`, `--network`, `-f`, `-b`, `--export-plan PLAN.yaml` |
| `plan` | `generate` (`-p PROFILE`: `security` (default), `windows-rdp`, `windows-winrm`, `windows-hostname`, `windows-domain-leave`, `windows-timezone`, `windows-static-ip`, `windows-dhcp`, `windows-dns`, `windows-sysprep`, `linux-ssh`, `linux-hostname`, `linux-grub`; `-o FILE`, `-f yaml\|json`, plus profile options such as `--user`, `--key`, `--key-file`, `--hostname`, `--workgroup`, `--timezone`, `--ip`, `--mask`, `--gateway`, `--dns`, `--interface-guid`, `--grub-timeout`, `--grub-cmdline`), `preview` (`-d`, `-s`), `validate`, `export` (`-f`), `apply` (`--vm`, `-n/--dry-run`, `-y`, `-i`, `-b DIR`, `--skip-backup`), `rollback` |
| `passport` | `emit --target T -o FILE [--bundle] [--content-hash] [--virtio-win DIR] [--live-url URL] [--sign-key FILE] [--issuer NAME] [--expires-hours H]`, `verify FILE [--fail-below S] [--require-signature] [--public-key HEX] [--trust-keys FILE] [--max-age-hours H]`, `handoff`, `keygen` |
| `fleet` | `analyze DIR`, `wave-plan DIR`, `watch DIR` (`--reset-baseline`, `--fail-on-drift`), `quarantine DIR` (`--threshold`, default 80, `--fail`); common flags `-o text\|json`, `--recursive`, `-j N` |
| `convert` | `-o PATH`, `-f qcow2\|raw\|vmdk\|vhd\|vdi`, `-c`, `-F`, `-P`, `--verify`, `-S`, `--preallocate`, `--compression-level 1-9`, `--buffer-size MB` |
| `shrink` | `--dry-run`, `--min-ratio` (default 3.0), `--headroom-pct` (default 20), `--json` |
| `qga` | `--socket PATH` (auto-discovered when omitted), `--execute CMD`, `--arguments JSON`, `--raw JSON` |
| `gate` | `--passport FILE` or `--image`, `--target`, `--fail-below` (default 80), `--sbom-old`, `--sbom-new`, `--rego FILE`, `-o` |

`guestkit-qemu` has three subcommands: `plan IMAGE`, `run IMAGE` (`--min-boot-score`, default 70; `--allow-unready`) and `qmp --socket PATH status|pause|resume|powerdown|quit|balloon MIB`. See [qemu-runtime.md](../features/qemu-runtime.md).

### What the release binary includes

The GitHub Release tarballs (`guestkit-<version>-linux-amd64.tar.gz` and the static `-musl` variant) are built with the `guestkit` binary only. The glibc build uses the default features (plus `registry-write` when libhivex was available at build time); the musl build uses `--no-default-features --features guest-inspect` (no libhivex, no libsystemd). Some commands exist in the CLI but need a feature that those builds do not enable:

| Feature | Needed for |
|---------|------------|
| `agent` (Unix) | `qga`, `agent`, `agent-proxy`, `agent-call`, `agent-inject`, `agent-sign`, `passport keygen`, `passport emit --sign-key`, and the `--agent-binary` / `--agent-unit` options of `repair --fix boot` and `migrate-plan` |
| `ai` | `doctor --ai`, `migrate-plan --ai`, `ai` |
| `mcp` | `mcp-serve` (or use the `ghcr.io/zyvorai/guestkit-mcp` image) |
| `registry-write` | Offline Windows registry hive writes: the AES SAM hash path of `rescue -o reset-password` on Windows, `agent-inject --windows`, and (together with `agent`) offline `DriverInject` of VirtIO drivers when a plan is applied (`plan apply`). Without both features a `DriverInject` operation is skipped with a warning. glibc tarball only, and only if libhivex was present at build time |
| `cloud-s3`, `cloud-azure`, `cloud-gcs` (`cloud` for all three) | `s3://`, Azure and `gs://` disk sources, see [cloud-disk-sources.md](../guides/cloud-disk-sources.md) |

Rebuild with `cargo build --release --features agent,ai,mcp,cloud` (or the subset you need). The in-guest agent has its own artifact: `zyvor-vm-tools-linux-amd64.tar.gz` and the `.deb` on the same release.

## Safety behaviour you should know about

These are the v1.2.11 behaviours that affect how you run the CLI against images you do not fully trust. Details are in the [changelog](../development/CHANGELOG.md).

- **Chroot-style path resolution.** Guest paths handed to the file operations (`cat`, `list`, `extract`, `copy`, `plan apply`, and so on) are resolved inside the guest root. An absolute guest symlink restarts at the guest root instead of pointing at your host, `..` never climbs above the root, and a dangling absolute symlink can no longer make a write land on a host path. File names that merely contain `..` (for example `app..log`) are accepted.
- **`nodev,nosuid` mounts.** Every host `mount` of a guest filesystem adds `nodev,nosuid` (appended last, so it overrides a caller-supplied `dev` or `suid`). Device nodes planted in an image cannot open host devices through the mount.
- **`cat` and file reads.** At most 1 GiB is read, and FIFOs, device nodes and directories are refused (a planted FIFO used to block forever).
- **`grep`.** The pattern is passed after `-e` and `--`, so a pattern starting with `-` is searched for, not parsed as a `grep` option.
- **Mount points and archives.** `..` in a mount point is rejected; tar/archive directory arguments are confined to the guest root; `cpio` extraction refuses absolute and `..` member names.
- **`glob_expand`, `grub_update`, `selinux_relabel`** use the guest root mount, never an arbitrary mount; glob patterns containing `..` are rejected and the results are guest paths.
- **Image paths** are made absolute before they are handed to `losetup`, `qemu-nbd` and `qemu-img`, so a file named `-x.img` or `json:{...}` is not read as an option or protocol spec.
- **NBD allocation lock** lives in `/run/lock/guestkit-nbd.lock`, then `$XDG_RUNTIME_DIR` or `/run/user/<uid>`; never `/tmp`. See [Troubleshooting](troubleshooting.md).
- **Binary cache** directory is owner-only (0700); the fallback is `/tmp/guestkit-<uid>` and is refused when it is a symlink or owned by someone else.

`guestkit execute` (`exec`) is different: it runs the command with `chroot` into the mounted guest root on **your host**, as root, so it executes the image's own binaries. Only use it on images you trust.

## Useful environment variables

| Variable | Purpose |
|----------|---------|
| `GUESTKIT_PACKAGE_CACHE` | Host dirs of `.rpm`/`.deb` for offline PackageInstall staging |
| `GUESTKIT_PACKAGE_FETCH` | `1`/`true` — host-download missing packages before staging |
| `GUESTKIT_PACKAGE_MIRROR` | HTTP base URL(s) for package fetch fallback (`curl`/`wget`) |
| `GUESTKIT_VIRTIO_WIN` | VirtIO driver tree for offline Windows DriverInject |
| `GUESTKIT_FLEET_JOBS` | Parallelism for `fleet analyze` (default min(4, CPUs)) |
| `GUESTKIT_FLEET_BASELINE_DIR` | Where `fleet watch` stores per-VM golden baselines |
| `GUESTKIT_S3_ENDPOINT` / `AWS_ENDPOINT_URL` | Custom S3-compatible endpoint for cloud disk pulls |
| `GUESTKIT_CLOUD_CACHE_DIR` | Override `~/.cache/guestkit/cloud/` |
| `GUESTKIT_QEMU_IMG` | Path to the `qemu-img` binary used by `guestkit img` |
| `GUESTKIT_DEBUG`, `GUESTKIT_TRACE` | Set to `1` for debug or trace logging in the engine |

The `--debug` global flag sets `GUESTKIT_DEBUG` for you; see the caveat under [Global flags](#global-flags) for the others.

Details: [fix-plans.md](../features/fix-plans.md), [cloud-disk-sources.md](../guides/cloud-disk-sources.md).

## Disk formats

QCOW2, VMDK, VDI, VHD, VHDX, RAW, IMG — detected from the file header (not the extension; v1.2.10 added VHDX, VDI, dynamic VHD and VMDK descriptor images). RAW/IMG/ISO use loop devices; the other formats use `qemu-nbd` and the `nbd` kernel module. Use **`--debug`** to see which path is used. Attaching a disk needs root (GuestKit uses `sudo` when it is not root); on Ubuntu 26.04 the device nodes are `root:disk 0660`, see [Prerequisites](../prerequisites-and-build.md#device-node-permissions-ubuntu-2604-and-other-recent-kernels).

`guestkit convert` writes `qcow2`, `raw`, `vmdk`, `vhd` or `vdi`. The worker `convert` job (see [offline repair worker](../devops/02-offline-repair-worker.md)) accepts `raw`, `qcow2`, `qcow`, `vmdk`, `vdi`, `vhd`, `vpc`, `vhdx`, `qed` and `parallels`, and refuses an output path equal to the source image.

## JSON output

Most inspect commands accept **`-o json`** or **`--json`** for scripting. See [quick-reference.md](quick-reference.md) for examples.

## See also

- [Documentation index](../INDEX.md)
- [Changelog](../development/CHANGELOG.md)
- [Roadmap](../development/roadmap.md)
- [zyvor.dev/guestkit](https://zyvor.dev/guestkit)
