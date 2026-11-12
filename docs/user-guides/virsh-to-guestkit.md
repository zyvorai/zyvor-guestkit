# Dump virsh — use GuestKit

GuestKit is the offline + live *guest* tool. It inspects disks, scores
boot readiness, repairs guests, and speaks the QEMU guest-agent socket.
For **host-local run + network**, use **[FluxVM](https://github.com/zyvorai/fluxvm)**
as the libvirt/virsh replacement (`create` / `get` / `delete`, TAP/netns/DHCP).
`guestkit vm` remains a lab-only smoke path (user-mode). KubeVirt / cluster
domains stay with `virtctl` / Machina.

This page is the cut-over map so scripts can delete every `virsh`
invocation.

`guestkit qga`, `agent-call` and `agent-proxy` are compiled only with `--features agent` (Unix). The release tarball does not include that feature, so build with `cargo build --release --features agent` for the live-guest rows below.

## Guest / disk operations (use GuestKit)

| Old virsh / libvirt habit | GuestKit |
|---|---|
| `virsh qemu-agent-command $vm '{"execute":"guest-ping"}'` | `guestkit qga --socket $SOCK --execute guest-ping` |
| `virsh qemu-agent-command $vm '{"execute":"guest-exec",...}'` | `guestkit qga --socket $SOCK --execute guest-exec --arguments '{"path":"/bin/true"}'` |
| `virsh qemu-agent-command $vm '{"execute":"guestkit-rpc",...}'` | `guestkit agent-call --socket $SOCK --method guestkit.getVersion` |
| Guess “will it boot?” by `virsh start` + `virsh console` | `guestkit doctor disk.qcow2 --target kvm --explain` |
| Hand-edit fstab after `virt-v2v` | `guestkit migrate-plan disk.qcow2 --target kvm --export plan.yaml` then `guestkit plan apply` |
| `guestfish` / libguestfs appliance | `guestkit inspect disk.qcow2`, `guestkit extract`, `guestkit rescue` |
| `virsh dumpxml` just to find the disk path | `guestkit domain-disks domain.xml` |
| `qemu-img check\|info\|snapshot` | `guestkit img check\|info\|snapshots` |
| Hunt virtio-win ISO by hand | `guestkit virtio-win plan --image win.qcow2` |
| `virsh start` then squint at console | `guestkit firstboot disk.qcow2 --target kvm --fail-below 80` |
| Serial console “did it boot?” | `guestkit agent-call --method guestkit.getBootAnalysis` |
| Snapshot FS freeze | GuestKit agent implements `guest-fsfreeze-freeze` / `thaw` on the QGA channel |

Typical libvirt QGA socket:

```text
/var/lib/libvirt/qemu/channel/target/<domain>/org.qemu.guest_agent.0
```

KubeVirt virt-launcher (inside the `compute` container):

```text
/var/run/kubevirt-private/libvirt/qemu/channel/target/org.qemu.guest_agent.0
```

`guestkit qga` auto-discovers those paths when `--socket` is omitted.

## Hypervisor object lifecycle

| Old virsh | Replacement |
|---|---|
| `virsh list --all` (local QEMU) | **FluxVM** `fluxvm list` ([zyvorai/fluxvm](https://github.com/zyvorai/fluxvm)); lab-only: `guestkit vm list` |
| `virsh define` / `start` / `shutdown` / `destroy` (local QEMU) | **FluxVM** `create` / `delete` (TTL); lab-only: `guestkit vm …` |
| Guest IP / libvirt NAT (`virbr0`) | FluxVM `user` / `tap`+bridge / `netns` (`fluxvm get` → `guest_ip`) |
| `virsh list --all` (KubeVirt) | `virtctl get vmi -A` / `kubectl get vmi -A` or Machina |
| `virsh start` / `shutdown` / `destroy` (KubeVirt) | `virtctl start\|stop\|restart` |
| `virsh console` | `virtctl console` or SPICE/VNC via Machina |
| `virsh define` / `undefine` / `dumpxml` (cluster) | KubeVirt VMI/VM YAML, or Machina domain API |
| `virsh migrate --live` | KubeVirt live migration / Machina |
| `virsh net-*` / `pool-*` | FluxVM network modes (host-local) or CNI + Atlas (cluster) |

**Suite rule:** GuestKit certifies the disk; FluxVM replaces libvirt for
host-local run + network; KubeVirt keeps `virtctl`.

See [VM lifecycle](../features/vm-runtime.md) and the FluxVM README
“Libvirt / virsh replacement” section.

## zyvor-api behaviour

`crates/zyvor-api/src/kubevirt_qga.rs` execs a socket client inside
virt-launcher. Order:

1. `guestkit qga --raw …`
2. `python3` / `python` unix-socket client
3. `perl` `IO::Socket::UNIX`
4. `socat` / `nc -U`
5. `virsh qemu-agent-command` **only if** `GUESTKIT_ALLOW_VIRSH=1`

## Script rewrite examples

```bash
# before
virsh qemu-agent-command web01 '{"execute":"guest-ping"}'

# after
guestkit qga --execute guest-ping
# or, pinned:
guestkit qga --socket /var/lib/libvirt/qemu/channel/target/web01/org.qemu.guest_agent.0 \
  --execute guest-ping
```

```bash
# before
virsh start web01 && virsh console web01

# after
guestkit doctor /var/lib/libvirt/images/web01.qcow2 --target kvm --explain
guestkit passport emit /var/lib/libvirt/images/web01.qcow2 --target kvm -o passport.json
# Then run with FluxVM (libvirt replacement):
#   fluxvm create --spec examples/guestkit-handoff.json
# Lab-only without FluxVM:
guestkit vm define web01 /var/lib/libvirt/images/web01.qcow2 && guestkit vm start web01 --force
# or KubeVirt/Machina domain, then:
guestkit agent-call --method guestkit.getBootAnalysis
```

## See also

- [Guest agent](../features/guest-agent.md)
- [Guest Control Fabric](../features/guest-control-fabric.md)
- [CLI quick reference](quick-reference.md)
- [QEMU / VirtIO runtime](../features/qemu-runtime.md)
- [VM lifecycle](../features/vm-runtime.md)
- [Architecture overview](../architecture/overview.md)
