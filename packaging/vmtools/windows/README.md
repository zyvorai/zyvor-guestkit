# Zeus VM Tools — Windows (Phase 4)

Windows guest tools ship via **virtio-win** today for QEMU Guest Agent. Native Zeus VM Tools MSI packaging is scaffolded here.

## Current path (QGA / virtio-win)

1. Open Zeus OS → VM → **Guest Tools**
2. Attach `virtio-win.iso`
3. Install QEMU Guest Agent / virtio-win guest tools inside the VM

## Native agent scaffold

| Artifact | Purpose |
|----------|---------|
| `install.ps1` | Download agent + register Windows service |
| `zyvor-guest-agent.wxs` | WiX MSI template |
| `build-msi.sh` | Cross-compile agent, build MSI or zip fallback |

Build on a host with Rust + optional WiX:

```bash
VERSION=0.1.0 ./packaging/vmtools/windows/build-msi.sh
```

Install inside the guest (Administrator):

```powershell
$env:ZYVOR_AGENT_URL = "http://<bundle-url>/windows/zyvor-guest-agent.exe"
.\install.ps1
```

Full parity with Linux `zyvor-guest-agent` (RPC, heartbeat, quiesce) is tracked separately; virtio-win remains the supported path until the Windows service reaches feature parity.
