# 03 — Air-gap packages & VirtIO

**Goal:** Stage Linux packages and Windows VirtIO bits into disks when guests cannot reach the internet on first boot.

---

## 1. Environment knobs

| Variable | Purpose |
|----------|---------|
| `GUESTKIT_PACKAGE_CACHE` | Host dirs of `.rpm`/`.deb` |
| `GUESTKIT_PACKAGE_FETCH` | `1` — host downloads missing pkgs before staging |
| `GUESTKIT_PACKAGE_MIRROR` | Comma-separated HTTP bases; optional `{name}` / `{ext}` |
| `GUESTKIT_VIRTIO_WIN` | VirtIO driver tree for offline `DriverInject` |
| `XDG_CACHE_HOME` | Influences default fetch cache under `…/guestkit/packages` |

```bash
export GUESTKIT_PACKAGE_CACHE=/opt/guestkit/pkgs
export GUESTKIT_PACKAGE_FETCH=1
export GUESTKIT_PACKAGE_MIRROR=https://mirror.corp.example/guestkit
export GUESTKIT_VIRTIO_WIN=/opt/virtio-win
```

---

## 2. Patterns

| Pattern | When |
|---------|------|
| **Cache only** | True air-gap; pre-seed RPMs/debs on the jump box |
| **Host fetch** | Worker has repo access; guest VLAN does not |
| **HTTP mirror** | Shared internal mirror for all cutover workers |

PackageInstall still needs resolvable package names in cache/dnf|apt/mirror — GuestKit does not invent RPMs.

Service enable/disable and CommandExec may stage **first-boot oneshots** when chroot cannot run live.

---

## 3. Mirror layout (suggested)

```text
https://mirror.corp.example/guestkit/
  centos9/qemu-guest-agent-….rpm
  ubuntu2204/qemu-guest-agent_….deb
  …
```

Document the mapping your plans expect. Version-pin weekend kits so every wave uses the same bits.

---

## 4. Windows VirtIO

```bash
# tree from Red Hat / Fedora virtio-win ISO or corp package
export GUESTKIT_VIRTIO_WIN=/opt/virtio-win
# include DriverInject in migrate-plan / day-0 pack as applicable
```

Offline `DriverInject` (applied by `plan apply`) is compiled in only with `--features registry-write,agent`. Without both, the operation is skipped with a warning. The GitHub Release tarballs and the `guestkit-worker` image do not enable `agent` (the worker links `guestkit` with its default features plus `guest-inspect`), so build `guestkit` from source with those features when you need offline VirtIO driver injection.

Without drivers staged offline, first boot on KVM often means yellow bangs and failed guest agent — fix before Passport floor if your policy requires it.

---

## 5. CI tips

- Bake cache into the worker AMI/image; don’t download mid-cutover from the public internet if policy forbids it.  
- Treat mirror as supply chain: checksum manifests, internal-only network.  
- Log which mode was used (cache/fetch/mirror) in the change ticket.  
