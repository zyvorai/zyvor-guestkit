# Zeus VM Tools

**Product name:** Zeus VM Tools  
**Binary:** `zyvor-guest-agent`  
**Package:** `zyvor-vm-tools` (tar.gz, DEB and ISO are attached to each GitHub Release; an RPM is built only when `rpmbuild` is present on the build host and is not a release asset)

Kubernetes-native guest agent, drivers bootstrap, and migration assurance for KubeVirt VMs — the VMware Tools equivalent for Zeus OS.

## Phase 1 capabilities

| Feature | Status |
|---------|--------|
| Linux agent (`zyvor-guest-agent`) | Shipped |
| `guestkit.getStatus` RPC (identity + heartbeat) | Shipped |
| Cloud-init install via zyvor-api | Shipped |
| Windows virtio-win path (Zeus deep link) | Shipped |
| Fleet coverage API | Shipped |
| CRDs (`VMToolsBundle`, `VMGuestAgent`, `VMToolsPolicy`, `GuestActionPolicy`) | Schema shipped |
| Windows MSI | Phase 4 |

## Phase 2 capabilities

| Feature | Status |
|---------|--------|
| Snapshot quiesce / unquiesce (`freeze` / `unfreeze`) | Shipped |
| Guest soft reboot (`softreboot`) | Shipped |
| Graceful shutdown (`stop` with grace period) | Shipped |
| QGA bootstrap install script (live qemu-ga) | Shipped |
| Fleet coverage UI chip | Shipped |
| Guest exec via API | **QGA guest-exec** via virt-launcher (`/vmtools/exec`, hands-off install) |

## Phase 3 capabilities

| Feature | Status |
|---------|--------|
| DEB package (`dpkg-deb`) | Shipped |
| ISO in `build-artifacts.sh` | Shipped (attached to releases) |
| RPM in `build-artifacts.sh` | Local builds only, when `rpmbuild` is installed; not published with releases |
| ISO attach install (`?method=iso`) | Shipped |
| `VMGuestAgent` CR status sync | Shipped |
| Windows MSI | Phase 4 scaffold |

## Phase 4 capabilities

| Feature | Status |
|---------|--------|
| Aurora dark UI theme (default) | Shipped |
| `VMToolsPolicy` GET/PUT/reconcile API | Shipped |
| Cluster auto-install reconciliation | Shipped |
| Reconcile CronJob when `autoInstall: true` | Shipped |
| `VMToolsBundle` CR + bundle URL from MinIO/Zeus | Shipped |
| Smarter reconcile (`pending` until agent connects) | Shipped |
| `autoUpgrade` rolling policy | Shipped |
| Fleet policy UI (auto-install + auto-upgrade + reconcile) | Shipped |
| Helm default `VMToolsPolicy` + CRD apply on deploy | Shipped |
| Windows MSI + install.ps1 scaffold | Scaffold (WiX on Windows build host) |
| Full standalone operator | CronJob reconcile (lightweight) |

## API routes

| Method | Path |
|--------|------|
| GET | `/api/v1/vmtools/bundle` |
| GET | `/api/v1/vmtools/coverage` |
| GET | `/api/v1/vmtools/policy` |
| PUT | `/api/v1/vmtools/policy` |
| POST | `/api/v1/vmtools/policy/reconcile` |
| GET | `/api/v1/kubevirt/vms/{ns}/{name}/vmtools` |
| POST | `/api/v1/kubevirt/vms/{ns}/{name}/vmtools/install` |
| POST | `/api/v1/kubevirt/vms/{ns}/{name}/inspect` |
| POST | `/api/v1/kubevirt/vms/{ns}/{name}/doctor` |
| POST | `/api/v1/kubevirt/vms/{ns}/{name}/vmtools/diagnostics` |
| POST | `/api/v1/kubevirt/vms/{ns}/{name}/vmtools/quiesce` |
| POST | `/api/v1/kubevirt/vms/{ns}/{name}/vmtools/unquiesce` |
| POST | `/api/v1/kubevirt/vms/{ns}/{name}/vmtools/reboot` |
| POST | `/api/v1/kubevirt/vms/{ns}/{name}/vmtools/shutdown` |
| POST | `/api/v1/kubevirt/vms/{ns}/{name}/vmtools/exec` |
| GET | `/api/v1/kubevirt/vms/{ns}/{name}/guest/info` |
| GET | `/api/v1/kubevirt/vms/{ns}/{name}/guest/systemd` |
| GET | `/api/v1/kubevirt/vms/{ns}/{name}/guest/logs` |
| POST | `/api/v1/kubevirt/vms/{ns}/{name}/guest/actions/restart-unit` |
| POST | `/api/v1/kubevirt/vms/{ns}/{name}/guest/actions/collect-support-bundle` |
| POST | `/api/v1/kubevirt/vms/{ns}/{name}/guest/actions/pre-snapshot-freeze` |
| POST | `/api/v1/kubevirt/vms/{ns}/{name}/guest/actions/post-snapshot-thaw` |
| POST | `/api/v1/guest-agents/register` |
| POST | `/api/v1/guest-agents/{id}/heartbeat` |
| POST | `/api/v1/guest-agents/{id}/report` |

Install query params: `restart=true|false`, `method=auto|cloud-init|qga|iso`

## Install methods

1. **Cloud-init** — **Install VM Tools** (Linux; merges cloud-init + virtio channel)
2. **QGA bootstrap** — when qemu-ga is connected, returns `bootstrap_script` for console install
3. **QGA file bootstrap** — airgap: API writes `zyvor-vm-tools` tar via `guest-file-write`, then `guest-exec` install (no guest network); see [guest-control-fabric.md](guest-control-fabric.md)
4. **ISO attach** — `?method=iso` creates CDI DataVolume, attaches CD-ROM, guest runs `/linux/install.sh`
5. **Offline inject** — `guestkit repair --inject-agent` or `POST .../guest/repair-plan` (GuestKit)

## Build packages

```bash
./packaging/vmtools/build-artifacts.sh
```

Produces `dist/vmtools/linux/` (tar.gz, deb, and an rpm only if `rpmbuild` is installed) and `dist/vmtools/zyvor-vm-tools.iso`. A release uploads the tar.gz (+ `.sha256`), the `.deb` and the ISO; the RPM is not uploaded. The bundle API (`GET /api/v1/vmtools/bundle`) still derives a `linux_rpm_url` from the bundle base URL, but nothing in the release publishes a file there, so treat it as present only if you uploaded one yourself.

Publish to MinIO:

```bash
MINIO_ENDPOINT=http://minio:9000 ./deploy/scripts/publish-vmtools-bundle.sh
```

Apply CRDs:

```bash
kubectl apply -f deploy/crd/zeus-vmtools.yaml
```

## VM labels (synced by zyvor-api)

```yaml
metadata:
  labels:
    zeus.zyvor.dev/guest-tools: connected|pending|missing
  annotations:
    zeus.zyvor.dev/tools-version: "0.1.0"
    zeus.zyvor.dev/last-heartbeat: "..."
```

`VMGuestAgent` CR `{vm-name}-vmtools` is upserted in the VM namespace with live status (`guestHealth`, `healthScore`, `failedUnits`, `systemdState` from agent push when `namespace`/`vm_name` are registered).

`GuestActionPolicy` CRD (`deploy/crd/zeus-guest-action-policy.yaml`) defines cluster-wide remediation allowlists and approval requirements (enforcement in agent policy is incremental).

## Windows (separate track)

Linux ships first via cloud-init, QGA bootstrap, or ISO. Windows VMs continue to use **virtio-win** for QEMU Guest Agent; native Zeus agent MSI is scaffolded:

```bash
./packaging/vmtools/windows/build-msi.sh
```

Inside the guest (Administrator PowerShell):

```powershell
$env:ZYVOR_AGENT_URL = "https://…/zyvor-guest-agent.exe"
.\install.ps1
```

See [packaging/vmtools/windows/README.md](../../packaging/vmtools/windows/README.md).

## Auto-reconcile

When `VMToolsPolicy.spec.autoInstall` or `autoUpgrade` is true, Helm can install a CronJob (`vmtools-reconcile`) that POSTs to `/api/v1/vmtools/policy/reconcile` on a schedule. Manual reconcile is available from the fleet toolbar.

QGA-bootstrap and ISO installs are labeled **pending** until `zyvor-guest-agent` connects; they are not counted as installed in fleet coverage.

See also [guest-agent.md](guest-agent.md) and [kubevirt-integration.md](kubevirt-integration.md).
