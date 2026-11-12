# Getting Started with GuestKit

Offline VM intelligence — inspect disks without booting, score boot readiness, and produce hypervisor-aware fix plans.

## What you need

| Requirement | Notes |
|-------------|--------|
| Linux host | qemu-img / losetup / qemu-nbd (`qemu-utils`) |
| Disk image | qcow2 / vmdk / raw (guest shut down for writes) |
| Optional UI | `:8088` zyvor-ui via GHCR compose |

## 1. Install

```bash
curl -fsSL -O https://github.com/zyvorai/zyvor-guestkit/releases/download/v1.2.11/guestkit-1.2.11-linux-amd64.tar.gz
# crates.io `guestkit` is still 0.3.2; releases ship tar.gz only (no RPM or COPR)
tar xzf guestkit-1.2.11-linux-amd64.tar.gz && cd guestkit-1.2.11-linux-amd64 && ./install.sh   # then run ./guestkit, or add this folder to PATH
export PATH="$PWD:$PATH"
qemu-img --version
guestkit version
```

## 2. First doctor run

```bash
guestkit doctor /path/to/disk.qcow2 --target kvm --explain
```

Optional TUI: `guestctl tui /path/to/disk.qcow2`.

## 3. Orient yourself

1. **Onboarding** — Getting Started, CLI Guide, Interactive Mode, TUI
2. **Inspection** — Inspect, Filesystems, Packages, Network, Files
3. **Assurance** — Doctor, Migration Plan, Policy, Passport, Fleet
4. **Fix** — Fix Plans, Repair / Rescue

## Next steps

- [Using the Dashboard](using-the-dashboard.md) (TUI / optional web)
- [Admin basics](admin-basics.md)
- [Common workflows](workflows.md)
- [Page guides](pages/README.md)
