# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
- **Python 3.10 is the minimum for `zyvor-guestkit`** (was 3.8; `abi3-py310`, wheel install test on 3.10 to 3.13).
- **CI: "Minimal Versions" replaced by "Latest Dependencies"** (advisory `cargo update` + `cargo check --all-features --all-targets` on stable).
- **k3s E2E** runs via `workflow_call` from `release.yml` after the GHCR images are published (90 minute timeout). Helm chart: UI targets 8443 over HTTPS, PostgreSQL readiness probe, new values `postgresql.memoryRequest`, `postgresql.securityContext`, `minio.command`; CI uses a pinned `bitnamilegacy` MinIO and an `Unconfined` AppArmor profile for PostgreSQL (Ubuntu 26.04 `cri-containerd` profile blocks `gosu` signalling).
- **Releases ship tar.gz only.** The `RPM Build` workflow (Mock builds, GitHub Release upload and COPR publish) is removed; the `.spec` files and `docs/development/RPM-BUILD.md` stay for anyone building locally. The Zeus VM tools `.deb` is a separate guest artifact and is unchanged.

### Documentation
- Install snippets, "current release" pins and GHCR/MCP tags updated from v1.2.10 (and older pins in `deploy/README.md`, `integration/README.md`) to v1.2.11; `scripts/check-doc-versions.py` reports docs that lag behind `Cargo.toml`.
- Tarball install steps (`tar xzf`, `./install.sh`) are documented, and the docs now say the release tarball holds `guestkit` and a `guestctl` symlink only: `guestkit-qemu` and `virtctl-guestkit` need a source build, and the `agent` (`qga`, `agent-*`, `passport keygen`), `ai`, `mcp`, `cloud-*` and offline `DriverInject` (`registry-write` + `agent`) features are not in the release binary.
- No RPM, COPR or `dnf install guestkit` is promised anywhere; `RPM-BUILD.md` and the Zeus VM Tools page say RPMs are local builds only.
- CLI reference rewritten against the clap definitions: real command names and aliases (there is no `cp`, `download`, `upload`, `profile` or `command-catalog`), global flags (and that `--read-only`, `--timeout` and `--cache-dir` are not read anywhere in the source), per-command flags, accepted targets.
- v1.2.11 behaviour documented: chroot-style guest path resolution, `nodev,nosuid` mounts, `read_file` limits, `grep` option injection fix, NBD lock location, job-id rules, worker convert allowlist, `zyvor-api` import hardening (CLI reference, troubleshooting, architecture overview, prerequisites, `SECURITY.md`).
- OpenAPI now covers every `zyvor-api` route (55 of the 115 paths were missing), declares path parameters, and describes authentication.
- Broken documentation links fixed; stale `getting-started.md` CLI examples (`convert --source`, `detect --image`, `-v`) corrected.

## [1.2.11] - 2026-11-12

### Changed (behaviour)
- Guest filesystems are mounted `nodev,nosuid`; device nodes inside a guest image are no longer usable through the mount.
- `read_file`/`cat` read at most 1 GiB and refuse non-regular files (FIFOs, device nodes, directories).
- The NBD allocation lock lives in `/run/lock` or `$XDG_RUNTIME_DIR` (`/run/user/<uid>`), never in `/tmp`.
- Worker job ids must match `[A-Za-z0-9._-]` (single path component, no leading `.`, at most 128 chars).

### Fixed
- **Guest path resolution** — guest paths are resolved chroot-style inside the guest root. Absolute guest symlinks are no longer re-evaluated against the host, a dangling absolute symlink can no longer make `write` reach a host path, and file names that merely contain `..` (for example `app..log`) are accepted.
- **Mount and archive traversal** — `mount` rejects `..` in mount points (it could create directories and mount outside the mount root); tar/archive operations confine their directory argument to the guest root instead of joining it onto an arbitrary mount.
- **Loop and NBD devices** — image paths are made absolute before `losetup`/`qemu-nbd` (a relative `-x.img` or `json:{..}` was parsed as an option or protocol spec); a connect failure no longer disconnects an NBD device that became busy under another owner; a loop device attached by `losetup` whose node never appears is detached instead of leaked.
- **Worker convert** — output formats are allowlisted and an output path that would overwrite the source image is refused.
- **`glob_expand`, `grub_update`, `selinux_relabel`** — no longer pick an arbitrary mount (`HashMap` order); they use the guest root mount like the other file operations. `glob_expand` now always resolves patterns against the guest root (never the host cwd), rejects `..`, drops hits that escape the root via symlinked directories, and returns guest paths instead of host paths.
- **NBD allocation lock** — no longer falls back to the predictable `/tmp/guestkit-nbd.lock`. It uses `/run/lock`, then `$XDG_RUNTIME_DIR` / `/run/user/<uid>`, opened with `O_NOFOLLOW`, mode 0600, and must be a regular file owned by the caller; failure is reported as an error.
- **`NbdDevice::disconnect`** — when the device stays busy it now keeps `connected` set (and the qemu-nbd child), so a retry or `Drop` attempts the disconnect again instead of leaking the device.
- **Job ids** — `JobValidator` rejects job ids that are not a single safe path component (`../`, `/`, leading `.`, >128 chars); the file transport also refuses them in `ack_job`/`nack_job`. Job ids were used to build spool/result file names.
- **`zyvor-api` storage import** — importing a file from a subdirectory of the storage root no longer overwrites an unrelated file already stored under the same name.
- **Guest mounts** — every host `mount` of a guest filesystem now adds `nodev,nosuid`. Without `nodev`, a device node planted in a malicious image (e.g. `/etc/shadow` as block device 8,0) was opened by the host kernel when guest files were read.
- **`read_file`/`cat`** — refuse FIFOs, device nodes and directories (opening a planted FIFO blocked forever) and files over 1 GiB instead of reading them into memory.
- **`grep`/`egrep`/`fgrep`** — the pattern is passed with `-e` and `--`, so a pattern starting with `-` is no longer interpreted as a `grep` option (e.g. `-f <host file>`).
- **`cpio_in`/`cpio_extract`** — refuse archives with absolute or `..` member names and pass `--no-absolute-filenames`, so an archive can no longer write outside the target directory. The archive is read with `-I` instead of being piped, which also fixes a deadlock in `cpio_extract`/`cpio_list` when the member list exceeded the pipe buffer.
- **GPT parsing** — a hostile header (`partition_entries_lba * 512` overflow, or a partition spanning `0..u64::MAX`) returns an error / saturates instead of panicking.
- **RAID inspection** — `/proc/mdstat` lines with brackets in reverse order (`]x[`) panicked the parser, and `[n/m]` counts were never read from the usual `[2/2] [UU]` line (the last bracket group was used).
- **Binary cache** — the directory is created owner-only (0700), the fallback is per-user (`/tmp/guestkit-<uid>`) and rejected if it is a symlink or owned by someone else, and concurrent saves of one key no longer share a temp file.
- **`zyvor-api` VM import** — the stored extension is allowlisted (it ended up in the object key and job payload) and a partially written upload is removed when the stream or the database insert fails.

## [1.2.10] - 2026-11-11

### Fixed
- **Disk images on current Ubuntu (26.04, kernel 7.x)** — loop and NBD devices attached through `sudo` are `root:disk 0660`, so the readiness check's unprivileged `open()` failed every time with "Loop device ... did not become ready in time". Readiness is now read from sysfs (`/sys/class/block/<dev>/size`, world-readable) first.
- **Disk format detection** — VHDX, VDI, dynamic VHD and VMDK descriptor images are recognised.
- **Package inspection** — apk package names and versions are split correctly.

### Security
- Cleared nine RustSec advisories by updating `rmcp` (3.1.2 → 3.9.0), `rustls-webpki`, `webpki-roots`, `zstd`, `zstd-safe` and `zstd-sys`; `evtx` 0.15.1 (RUSTSEC-2026-0431). `cargo audit` reports no vulnerabilities (only "unmaintained crate" notices).

### Changed
- All semver-compatible dependency updates in the root and per-crate lockfiles. Major-version updates are not included.
- `zyvor-api` and `guestkit-worker` lockfiles now carry `sonic-number` 0.1.3 (the image build failed to compile `sonic-rs` 0.5.10 against 0.1.2).
- CI: runs for superseded commits are cancelled; `actions/cache` v4; Ubuntu 26.04 device-node permissions and the `qemu-system-x86` package; bounded memory for the RPM builds.

## [1.2.9] - 2026-10-18

### Fixed
- **`zyvor-api` image build** — the Dockerfile now copies the eBPF workspace crates that `guestkit` depends on.

## [1.2.8] - 2026-10-18

### Fixed
- **Release workflow** — the self-hosted GHCR and MCP Registry jobs reclaim workspace ownership before checkout, so root-owned files from earlier container builds no longer break them.

## [1.2.7] - 2026-10-18

### Fixed
- **crates.io publish** — the release workflow now publishes `guestkit-ebpf-common` and `guestkit-ebpf-runtime` before `guestkit`; v1.2.6 could not be published because those crates were not on the registry.

## [1.2.6] - 2026-10-17

### Added
- **Per-container eBPF network policy and LSM MAC in the guest agent** (`guestkit.netpolicy` / `guestkit.lsm`).
- **MCP Registry packaging** — `guestkit mcp-serve` is packaged as an OCI image (`ghcr.io/zyvorai/guestkit-mcp`) with `server.json` for the MCP Registry; the MCP server and registry listing are documented.
- **Docs** — sales-ready README with capability, comparison and architecture cards, new hero and social cards, a voluntary donation link (Support section, page, `FUNDING.yml`).

### Fixed
- **`exists()` resolves symlinks inside the guest** — `Guestfs::exists` follows guest symlinks relative to the guest root instead of the host filesystem.

### Changed
- **Dependencies** — ratatui 0.30 and crossterm 0.29, which drops `lru` 0.12 and its RUSTSEC `IterMut` soundness advisory.
- **Docs site** — served from `/zyvor-guestkit/` after the organization rename, dark by default.
- **CI** — heavy trusted jobs run on self-hosted `zyvor-build` runners.

## [1.2.5] - 2026-10-03

### Fixed
- **Online snapshots failed at the filesystem freeze, and the agent claimed it had frozen.** `guestkitd` runs as `zyvor-agent` with an empty capability set, but the QGA handlers `guest-fsfreeze-freeze`/`-thaw` ran `fsfreeze` themselves, so every freeze failed with `Operation not permitted` and KubeVirt failed the snapshot of any VM with PVC-backed disks (measured on Ubuntu 24.04 under KubeVirt 1.9: a freshly installed agent cannot freeze; with `CAP_SYS_ADMIN` it can). The unit file already says privileged operations run in `guestkitd-exec`; freeze and thaw now do: a new `fsfreeze` action in the helper (`freeze`/`thaw` only, always `/`) and `executor_ipc::fsfreeze()` for the agent, falling back to an in-process call only when no helper is running (agent run as root). Also, `snapshot_hooks::freeze_filesystems`/`thaw_filesystems` treated a successfully *spawned* `fsfreeze` as success (`status().is_ok()`), so `snapshot.prepare` reported `fs_frozen: true` while nothing was frozen; they now use the same checked path. Covered by a test that starts the real `guestkitd-exec` with a fake `fsfreeze`. Requires the helper service (`zyvor-guest-agent-exec.service`, enabled by the packages) to be running.
- **Linux agent never connected to the hypervisor channel on stock distros.** The virtio port (`/dev/virtio-ports/org.qemu.guest_agent.0`) is `root:root 0600` but `guestkit-agent.service` runs as `zyvor-agent`, so the agent started, logged `failed to open virtio channel`, and KubeVirt reported no guest agent. The DEB, RPM and tarball now ship `60-zyvor-guest-agent.rules` (group `zyvor-agent`, mode 0660 for `org.qemu.guest_agent.0` and `com.zyvor.guestkit.0`) and reload/trigger udev on install. Verified on Ubuntu 24.04 under KubeVirt 1.9: with the rule the agent connects (`agentConnected`), without it it does not.
- **Concurrent NBD allocate+connect** — `NbdDevice::connect` holds a cross-process `flock` (`/run/lock/guestkit-nbd.lock`, fallback `/tmp`) across free-device selection and `qemu-nbd -c`, and retries the next free index on failure. Fixes races where two mounts claimed the same `/dev/nbdN` (fluxvm#104: `/dev/nbd0p1 already mounted`, wrong guest-agent token). `new()` no longer claims a device index before connect.

### Added
- **Offline inject on `run_migrate_repair`** — optional `inject_json` appends hostname, network files, users, services, first-boot scripts, cloud-init user-data, Active Directory rejoin, Windows KMS reactivation, and RDP enable to the repair plan. Empty or `"null"` is a no-op. `guestkit migrate-repair` has no inject flag; h2kvm passes the JSON from Python.
- **Live guest fix helpers** — `live_fix_commands()` returns shell lines (regenerate initramfs, update GRUB, optionally remove `open-vm-tools`). `run_live_plan(commands, dry_run=False)` runs those lines on the machine where Python is executing. It does not SSH.

## [1.2.4] - 2026-09-18

### Changed
- **Docs** — install instructions use the GitHub Release and `pip install zyvor-guestkit`. GHCR examples pin `v1.2.4`. Serial console without `grubby` is documented. crates.io `guestkit` is still 0.3.2.

## [1.2.3] - 2026-09-18

### Fixed
- **Serial console without `grubby`** — Photon and other guests have no `grubby`. Kernel console args are written into the bootloader the guest already uses (grub2, BLS, syslinux, extlinux, zipl) instead of failing first-boot with exit 127.
- **XFS remount** — pick a valid remount option set after `norecovery` inspect mounts.
- **Guest root resolution** — path resolve uses the real guest `/`, not `sda1`/`sda2`.
- **Windows agent inject** — mount the Windows root read-write for offline `guestkitd` inject.

### Changed
- **PyPI** — Python distribution is `zyvor-guestkit`, owned by [pypi.org/user/zyvor](https://pypi.org/user/zyvor/). `hypersdk-guestkit` is retired. Import name stays `guestkit`.
- **GHCR registry** — container images publish/pull from `ghcr.io/zyvorai/{zyvor-ui,zyvor-api,guestkit-worker}` (was `ghcr.io/hypersdk`).
- **Docs** — customer wording is now users; CE vs Enterprise licensing is spelled out; docs site is Docusaurus.

### Removed
- Zyvor company terms and the distribution acceptance gate.

## [1.2.2] - 2026-09-06

### Added
- **Web Image Vault TUI parity** — OSS `deploy/ui` detail workspace: inspect inventory tabs, Assurance (plan/passport/repair preview + gated apply), Profiles, and Files browse.
- **API / worker** — `POST /api/v1/vms/:id/profile`, `POST /api/v1/vms/:id/explore` (`guestkit.explore` ls/stat/cat), repair `dry_run` query; larger inspect samples.

## [1.2.1] - 2026-09-03

### Security
- Patched RUSTSEC-2026-0176/0177 (pyo3 0.27.2 -> 0.29.2: out-of-bounds read in `PyList`/`PyTuple` iterators, missing `Sync` bound on `PyCFunction::new_closure` closures) and RUSTSEC-2026-0258 (h2 0.4.14 -> 0.4.16: unbounded empty DATA frames). `cargo audit` now reports zero vulnerabilities.
- Verified the pyo3 bump beyond `cargo check`: a real `maturin build --release --features python-bindings` produced a working wheel, installed cleanly into a fresh venv, and `import guestkit` reports the right version -- no breaking API changes hit this crate's bindings code between 0.27 and 0.29.

## [1.2.0] - 2026-09-03

### Added
- **GuestKit-assured QEMU/VirtIO runtime and `guestkit-qemu` CLI**, replacing `virsh qemu-agent-command` with direct QGA socket access.
- **`guestkit vm`** — local QEMU lifecycle management without libvirt.
- **`virtctl-guestkit guestfs`** — a drop-in for `virtctl guestfs`.
- **Cutover bundle**: gate, SELinux/sysprep/BitLocker handling, virtio-initramfs, agent-sign.
- **Cloud cutover profiles**, Rego policy checks, and an offline cloud-init datasource.
- **Passport handoff, fleet quarantine**, and the `virtctl-guestkit` plugin.
- **`guestkit img`, domain-disks, virtio-win, and firstboot** commands.
- `sbom-diff`, rescue dry-run Action, and Passport Action extras.
- **`guestkit shrink <image>`** -- shrinks an oversized-but-mostly-empty guest disk's declared virtual size to match its real data footprint, so it stops looking far bigger than it actually is to downstream tools. Motivated by a real KubeVirt/CDI import failure: a VMware "growable" VMDK declared at 500GB virtual holding 6GB of actual data uploaded successfully, then was rejected by CDI at the finish line because the cluster's storage didn't have 500GB free -- CDI requires the destination to fit the full declared virtual size regardless of real usage.
  - `--dry-run` -- report what would happen without changing anything.
  - `--min-ratio N` (default `3.0`) -- only shrink when virtual/actual size ratio is at least this much.
  - `--headroom-pct N` (default `20`) -- extra space left over the filesystem's true minimum size.
  - `--json` -- machine-readable output, consumed by hyper2kvm's `--shrink-oversized-disk` pipeline step.
  - v1 scope is intentionally narrow for safety: a single/last ext2/3/4 partition, MBR or GPT, no LVM/LUKS -- any other layout is reported and left untouched, never a hard error and never a guess, since this operation mutates the guest filesystem and partition table.
  - **Implementation: build a fresh, correctly-sized destination and copy files across (`rsync -aHAX`), not an in-place `resize2fs` shrink.** In-place shrink was tried first and rejected by measurement: `resize2fs` relocates data per-inode, so a real OS install (tens of thousands of files scattered across block groups sized for the original huge nominal capacity) took 40+ minutes and was still running, vs. ~50s for the same nominal size with one large test file -- the cost is dominated by how scattered the real data is, not its total size. The copy-based approach's cost scales with actual bytes copied (a couple of minutes for ~6GB), matching what `virt-resize` does internally. No new dependency on the libguestfs appliance -- built from the same lightweight tools (`resize2fs`, `sgdisk`, `mke2fs`, `rsync`, `dd`, `qemu-img`) guestkit already shells to elsewhere.
  - GPT disks are resized via `sgdisk -d/-n` (delete + recreate the partition, new bounds and backup-header repair in one write) rather than `parted resizepart` + `sgdisk -e`: after copying the original partition table bytes into the smaller destination container, the copied primary GPT header still points its backup/AlternateLBA at the *original* disk's end -- `parted` can't even open the disk to inspect it ("Invalid argument during seek for read"), and `sgdisk -e` alone refuses too ("Problem: partition 2 is too big for the disk. Aborting write operation!", since it validates every partition against the current device size before writing anything, and the still-huge partition 2 fails that check even though fixing the header is what would resolve it). Deleting and recreating the partition at its new, valid bounds sidesteps that chicken-and-egg validation entirely.
  - New `Guestfs::disk_actual_size()` and `Guestfs::disk_resize_shrink()` in `guestfs::disk_mgmt`.

### Fixed
- **`disk_virtual_size()` / `disk_format()` returned the wrong value for VMDK and other formats with a nested `children` block in `qemu-img info --output=json`** -- both parsed the JSON by scanning raw text for the first line containing `"virtual-size"`/`"format"`, but qemu-img's JSON nests a `children[].info` object (the underlying file's own size/format) that repeats those same keys *earlier* in the text than the real top-level values. For a VMDK this silently returned the file's raw byte length (e.g. 6GB) instead of its declared virtual size (e.g. 500GB) -- caught while building the `shrink` command above, whose whole purpose is telling those two numbers apart. Both now parse the full JSON via `serde_json` and read only the top-level keys.
- **Two simultaneous `Guestfs` handles in one process silently mounted on top of each other, discarding one filesystem's contents** (`guestfs::mount::create_mount_root`) -- the per-handle mount directory was named `guestkit-<pid>` using only the process ID, so a second handle created in the same process (needed by `shrink` for its simultaneous source + destination filesystems) computed the *identical* path; `fs::create_dir_all` on an already-existing directory succeeds silently, so the second `mount()` call mounted its filesystem directly on top of the first's, shadowing it. Caught by verifying `shrink`'s output disk's actual contents rather than trusting its "success" log line -- the very first working end-to-end run had silently produced an empty destination filesystem (`lost+found` only, all real data discarded). Fixed with a per-process atomic counter appended to the directory name; every prior caller only ever used one handle at a time, so this was previously latent.
- `guestfs::mount()` now remounts read-write when a caller's mount hits an already-mounted device, instead of failing.
- Gate passport writes on non-writable image directories; companion CLIs are now installed on deploy.
- BOOT-003 false positive when `/boot` is a separate fstab partition.
- `guestkit qga_client` build failure (missing `FileTypeExt` import for `is_socket`).
- Stopped flagging `open-vm-tools` as a proprietary VMware remnant.
- Patched RUSTSEC-2026-0185 (quinn-proto, remote memory exhaustion via unbounded out-of-order stream reassembly) and cleared rustfmt drift that was blocking CI.
- Various stale CLI names, dead links, and a license contradiction in the docs.

### Security
- RUSTSEC-2026-0185 (see above).

### Docs
- Documented the split: GuestKit certifies disks, Ephemera (now FluxVM) runs and manages VMs — including pointing the suite handoff and virsh guide at FluxVM.
