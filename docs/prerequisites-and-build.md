# Prerequisites and build

> Part of the [GuestKit README](../README.md). Back to the [documentation map](documentation-map.md).

# Prerequisites

- **Running GuestKit:** Linux with `qemu-img`, `losetup` and `qemu-nbd` (Debian/Ubuntu: `qemu-utils` and `util-linux`; Fedora/RHEL: `qemu-img` and `util-linux`). Attaching a disk needs root: GuestKit runs `losetup`, `qemu-nbd`, `modprobe`, `mount` and `umount` through `sudo` when it is not already root. The web console and worker run anywhere Docker does — see [Run the free web stack](web-stack-ghcr.md).
- **Kernel modules:** `loop` (raw, img, iso images) is built in or loaded on demand. `nbd` (qcow2, VMDK, VDI, VHD, VHDX) is loaded by GuestKit with `modprobe nbd max_part=16` when it is missing; if that fails, load it yourself and make it persistent with `echo nbd | sudo tee /etc/modules-load.d/nbd.conf`. Containers need `--privileged` (or `SYS_ADMIN` plus the loop/NBD devices).
- **Building from source:** a stable Rust toolchain (edition 2021) and the libsystemd development headers, which the default `journal-native` feature links. Optional features (`ai`, `mcp`, `registry-write`, the Windows agent) have extra requirements — see the `Makefile` and [CONTRIBUTING](development/CONTRIBUTING.md).
- **Python bindings:** `pip install zyvor-guestkit` needs nothing else; building them yourself uses `build_python.sh`.
- **Installing a release:** the GitHub Release tarballs (`guestkit-<version>-linux-amd64.tar.gz` and the static `-musl` build) hold `guestkit`, a `guestctl` symlink, `install.sh` (installs `qemu-utils`/`nbd` through `apt` or `dnf`, then verifies the binary), test scripts and legal files. There are no RPM or COPR packages and no `dnf install guestkit`. `guestkit-qemu` and `virtctl-guestkit` are not in the tarball; the `agent`, `ai`, `mcp` and `cloud-*` features are not compiled into it either, see [CLI reference](user-guides/cli-guide.md#what-the-release-binary-includes).

## Device-node permissions (Ubuntu 26.04 and other recent kernels)

On current kernels (for example Ubuntu 26.04, kernel 7.x) `/dev/loopN` and `/dev/nbdN` are created on demand and are `root:disk` mode `0660`. A process that is not root and not in the `disk` group cannot open them, even though the device attached successfully. Since v1.2.10 GuestKit checks readiness through `/sys/class/block/<dev>/size`, which is world-readable, so the attach itself no longer times out. Reading or mounting the device still needs privilege:

- run GuestKit as root (`sudo guestctl ...`), or
- add your user to the `disk` group (`sudo usermod -aG disk $USER`, then log in again), or
- install a udev rule that gives a group of your choice access, for example `/etc/udev/rules.d/99-guestkit-devices.rules`:

  ```text
  KERNEL=="loop[0-9]*", GROUP="disk", MODE="0660"
  KERNEL=="nbd[0-9]*", GROUP="disk", MODE="0660"
  ```

  then `sudo udevadm control --reload-rules`. Disposable CI runners may use `MODE="0666"`; do not do that on a shared host, because it lets every local user read any attached disk.

## What GuestKit does to the host (v1.2.11)

- Guest filesystems are mounted with `nodev,nosuid` appended to the mount options, so device nodes inside an image cannot reach host devices and setuid bits are ignored.
- The NBD allocation lock is `/run/lock/guestkit-nbd.lock` (as root), else `guestkit-nbd.lock` in `$XDG_RUNTIME_DIR` or `/run/user/<uid>`. It is never created in `/tmp`; if no candidate directory works, the attach fails with `Failed to acquire NBD allocation lock`. See [Troubleshooting](user-guides/troubleshooting.md#errors-added-by-the-v1211-hardening).
- Image paths are made absolute before they reach `losetup`, `qemu-nbd` and `qemu-img`.
- Process disk images you do not trust in a container or VM; mounting always needs root.

# Build

The `.spec` files in the repository (`guestkit.spec`, `guestkit-full.spec`) are for local RPM builds only; see [RPM-BUILD.md](development/RPM-BUILD.md). Releases do not ship RPMs.

```bash
cargo build --release     # guestkit, guestctl, guestkit-qemu, virtctl-guestkit
cargo build --release --features agent,ai,mcp,cloud   # optional: qga/agent-*, --ai, mcp-serve, S3/Azure/GCS sources
cargo test
make check                # tests + clippy
make install              # installs the guestkit binary (PREFIX=$HOME/.local to change the location)
```

See [CONTRIBUTING](development/CONTRIBUTING.md) and CI under `.github/workflows/`. **`docs/` and this README are authoritative.**
