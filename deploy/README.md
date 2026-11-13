# Zyvor VM Services on Kubernetes

Deploy GuestKit workers, zyvor-api, and supporting services for offline VM intelligence and KubeVirt migration planning.

## Components

| Service | Description |
|---------|-------------|
| `zyvor-api` | REST gateway — import, inspect, doctor, migration-plan, KubeVirt boot inspect, provision |
| `guestkit-worker` | Redis Streams worker running GuestKit operations |
| `postgresql` | VM inventory and job metadata |
| `redis` | Job queue (`zyvor:jobs` stream) |
| `minio` | VM image object storage (optional; PVC path also supported) |
| `zyvor-ui` | Minimal web console |

## Quick start

```bash
# Prerequisites: kind, kubectl, helm, docker
./deploy/scripts/kind-kubevirt-quickstart.sh
```

## Helm install

```bash
helm install zyvor deploy/helm/zyvor -n zyvor --create-namespace
```

## k3s deploy (Ubuntu)

Single-node k3s on Ubuntu (remote host or CI):

```bash
# Installs k3s, helm, qemu-utils, podman; deploys the full stack
bash deploy/scripts/install-k3s-ubuntu.sh
bash deploy/scripts/deploy-remote-k3s.sh

# CI overlay (no hardcoded host IP — URLs computed at deploy time)
HELM_VALUES_FILE=values-ci.yaml bash deploy/scripts/deploy-remote-k3s.sh

# Pull release images from GHCR instead of local build
PULL_REGISTRY=ghcr.io/zyvorai IMAGE_TAG=v1.2.11 bash deploy/scripts/deploy-remote-k3s.sh
```

Override public URLs when the node IP is not the client-facing address:

```bash
NODE_IP=203.0.113.10 \
ZEUS_PUBLIC_URL=http://203.0.113.10:30080 \
VMTOOLS_BASE_URL=http://203.0.113.10:30092/vmtools \
bash deploy/scripts/deploy-remote-k3s.sh
```

Helm overlays:

| File | Use |
|------|-----|
| `values-k3s.yaml` | Remote k3s (URLs via env/`--set`) |
| `values-ci.yaml` | GitHub Actions `k3s-e2e.yml` (pinned Bitnami-legacy MinIO, unconfined PostgreSQL AppArmor; see [CI k3s E2E](#ci-k3s-e2e)) |

## Fresh-cluster install notes

A bare `helm install zyvor deploy/helm/zyvor -n zyvor --create-namespace` will
**not** come up as-is on a stock cluster. The first-party images default to
unregistered `:latest`, and the default StorageClass (`longhorn`) usually does
not exist. A working from-scratch install (verified) overrides those:

```bash
helm install zyvor deploy/helm/zyvor -n zyvor --create-namespace \
  --set namespace=zyvor \
  --set zyvorApi.image=ghcr.io/zyvorai/zyvor-api:<ver> \
  --set zyvorUi.image=ghcr.io/zyvorai/zyvor-ui:<ver> \
  --set guestkitWorker.image=ghcr.io/zyvorai/guestkit-worker:<ver> \
  --set zyvorApi.storageClass=<sc> \
  --set persistence.vmImages.storageClass=<sc> \
  --set persistence.vmImages.accessMode=ReadWriteMany   # RWX for multi-node; RWO single-node
```

Notes:
- `.Values.namespace` is independent of `.Release.Namespace` — set both.
- KubeVirt + CDI, an Ingress controller, and (for the k3s overlay) a CephFS/RWX
  provider are **prerequisites**, not bundled.
- The KubeVirt ClusterRole/Binding are namespace-scoped
  (`zyvor-api-kubevirt-<ns>`), so multiple installs coexist.

## Production hardening (before real user data)

These are deployment-time decisions the chart leaves to the operator:

- **Auth**: off by default. Enable with `--set zyvorApi.auth.enabled=true` and a
  real `--set zyvorApi.auth.jwtSecret=$(openssl rand -base64 32)`. The API
  **fails closed** — it refuses to start if auth is on without a real secret.
- **Secrets**: change the default `postgresql.password` / `minio` keys. The DB
  DSN is delivered via the `zyvor-secrets` Secret (not plaintext env).
- **Persistence**: postgres and minio use `emptyDir` by default → **data is lost
  on pod restart**. Wire PVCs before production.
- **TLS**: the Ingress is HTTP-only (`ssl-redirect: false`); terminate TLS at
  your ingress / LB (e.g. cert-manager). Agent↔API is already mTLS on 8443.
- **Backup**: no built-in backup/restore for the DB or image vault — add your own
  (`pg_dump` CronJob, vault snapshot).

## Release artifacts (`v*` tags)

The [release workflow](../.github/workflows/release.yml) publishes:

| Asset | Destination |
|-------|-------------|
| `guestkit-<ver>-linux-amd64.tar.gz` and `guestkit-<ver>-linux-amd64-musl.tar.gz` (each with `.sha256`) | GitHub Release |
| `zyvor-vm-tools-linux-amd64.tar.gz` (+ `.sha256`), `zyvor-vm-tools_<ver>_amd64.deb`, optional `zyvor-vm-tools.iso` (guest agent, separate from the host CLI) | GitHub Release |
| `ghcr.io/zyvorai/guestkit-worker:v<ver>` | GHCR |
| `ghcr.io/zyvorai/zyvor-api:v<ver>` | GHCR |
| `ghcr.io/zyvorai/zyvor-ui:v<ver>` | GHCR |
| `ghcr.io/zyvorai/guestkit-mcp:<ver>` | GHCR (MCP Registry image) |
| `zyvor-guestkit` wheel | PyPI |

Releases ship tar.gz only: there are no RPM packages and no COPR repository.

Build images locally with `deploy/scripts/publish-images.sh`. The root `Dockerfile` is deprecated; use per-service Dockerfiles under `crates/` and `deploy/ui/`.

Manual agent-only rebuild: [agent-release.yml](../.github/workflows/agent-release.yml) (`workflow_dispatch`).

## CI k3s E2E

[`.github/workflows/k3s-e2e.yml`](../.github/workflows/k3s-e2e.yml) on `ubuntu-latest`, 90 minute job timeout (with per-step limits: deploy 45, VM tools bundle 30, E2E orchestrator 20 minutes). Triggers:

- **Pull requests** to `main` touching `crates/**`, `deploy/**`, `packaging/vmtools/**` or the workflow itself: images are built locally with podman.
- **`workflow_call` from [release.yml](../.github/workflows/release.yml)**: the `k3s-e2e` job runs after `publish-release-images`, so the `v*` GHCR images are guaranteed to exist and are pulled (`ghcr.io/zyvorai`, tag = release tag). The workflow has no tag trigger of its own.
- **`workflow_dispatch`** (manual).

Steps:

1. `install-k3s-ubuntu.sh` - k3s, helm, qemu-utils
2. `deploy-remote-k3s.sh` with `HELM_VALUES_FILE=values-ci.yaml` - waits for the rollouts, then for `pg_isready` inside the PostgreSQL pod and the API pod's `Ready` condition (so a database problem fails the job minutes in, not after the VM-tools build)
3. `publish-vmtools-bundle.sh` - agent binaries into in-cluster MinIO
4. `ci-k3s-e2e.sh` - API health, `e2e-smoke.sh` (cirros), VM tools bundle/API checks, and `curl -k https://<node>:30081/` against the UI (self-signed certificate)

On failure the job dumps pod logs, events, DNS and PostgreSQL diagnostics.

Run it yourself:

```bash
gh workflow run k3s-e2e.yml            # on a branch / main
gh run watch                           # follow it
# or against a host that already has k3s:
HELM_VALUES_FILE=values-ci.yaml bash deploy/scripts/deploy-remote-k3s.sh
bash deploy/scripts/ci-k3s-e2e.sh
```

### What `values-ci.yaml` changes

| Value | CI setting | Why |
|-------|-----------|-----|
| `minio.image` | `docker.io/bitnamilegacy/minio:2025.7.23-debian-12-r5` | `minio/minio` can no longer be pulled from Docker Hub or quay.io |
| `minio.command` | `["minio"]` | the Bitnami entrypoint is a wrapper; run the binary with the chart's `args` |
| `postgresql.securityContext.appArmorProfile.type` | `Unconfined` | see the AppArmor caveat below |

Other chart values added with these fixes (defaults in `values.yaml`): `postgresql.memoryRequest` (default `128Mi`, the pod's memory request), `postgresql.securityContext` (optional pod `securityContext`, default `{}`) and `minio.command` (optional command override, default `[]`). The PostgreSQL pod now has a `pg_isready` readiness probe, so rollouts and the API's first connection no longer race `initdb`. The `zyvor-ui` container and Service target port **8443**: the UI image serves HTTPS only (self-signed certificate generated at start-up with `openssl`, which the image now installs) and nothing listens on 80.

### Ubuntu 26.04 nodes: AppArmor caveat

On Ubuntu 26.04 (kernel 7.x, including GitHub's `ubuntu-latest`) the containerd `cri-containerd.apparmor.d` profile denies a container's processes signalling each other. The official `postgres` image starts through `gosu`, which then spins forever and `pg_ctl stop` fails with "Permission denied" (`dmesg`: `apparmor="DENIED" operation="signal"`). PostgreSQL never comes up and `zyvor-api` dies with "pool timed out". CI works around this by running the PostgreSQL pod with an `Unconfined` AppArmor profile; this is for CI only. On your own Ubuntu 26.04 nodes either fix the node's AppArmor/containerd policy or set the same override deliberately:

```bash
helm upgrade --install zyvor deploy/helm/zyvor -n zyvor \
  --set postgresql.securityContext.appArmorProfile.type=Unconfined
```

### Known issues

- `values.yaml` and `values-prod.yaml` still name `minio/minio:RELEASE.2024-12-18T13-15-44Z`, which is no longer pullable. Outside CI, override `minio.image` (and `minio.command` if the image's entrypoint is not `minio`), or use the CI overlay values as a template, or disable MinIO (`minio.enabled=false`) and use the PVC path.
- The Ingress (`templates/ingress.yaml`) sends plain HTTP to the `zyvor-ui` Service (port 80), which now forwards to the TLS-only UI on 8443, so the ingress route to the console is not expected to work until the chart is changed (for example `nginx.ingress.kubernetes.io/backend-protocol: HTTPS`). Use the NodePort over `https://` (self-signed) or `kubectl port-forward svc/zyvor-ui 8081:80` and open `https://localhost:8081`. Not verified against a live cluster.
- `deploy-remote-k3s.sh` still prints the UI URL as `http://...`; use `https://`.

**Ubuntu live E2E** (manual, on a k3s host with MinIO + KubeVirt):

```bash
API=http://<node-ip>:30080/api/v1 \
AGENT_NODE=http://<node-ip>:30092 \
bash deploy/scripts/e2e-ubuntu-k3s.sh
```

Imports Ubuntu 22.04, runs offline inspect/doctor, provisions a CDI VM with IP-only cloud-init (QGA deb + `zyvor-guest-agent` from MinIO), exercises **Guest Control Fabric** routes (`guest/status`, `guest/doctor`, airgap `guest/install-agent`), live guest intel, then offline cluster inspect/doctor on a halted VM.

**Airgap install:** when QGA is up but guest network is down, `POST .../guest/install-agent` with `strategy: auto` selects QGA file bootstrap (chunked `guest-file-write`, no curl in guest). See [guest-control-fabric.md](../docs/features/guest-control-fabric.md).

## API

See [openapi/zyvor-vm-services.yaml](openapi/zyvor-vm-services.yaml).

Example workflow:

```bash
API=http://api.zyvor.local/api/v1

# Import
curl -F "file=@disk.vmdk" $API/vms/import

# Doctor
curl -X POST "$API/vms/{id}/doctor?target=kubevirt&explain=true"

# Migration plan
curl -X POST "$API/vms/{id}/migration-plan?target=kubevirt"

# Provision KubeVirt YAML
curl -X POST "$API/vms/{id}/provision"

# Offline boot inspect (stopped KubeVirt VM — pure Rust GuestKit, not legacy appliance tooling)
curl "$API/kubevirt/vms/default/my-vm/boot-inspect"
curl -X POST "$API/kubevirt/boot-inspect" \
  -H 'Content-Type: application/json' \
  -d '{"namespace":"default","vm":"my-vm","mode":"boot-inspect","source":"zeus-os"}'
```

See [docs/features/kubevirt-integration.md](../docs/features/kubevirt-integration.md) for Zeus OS Guest Intelligence integration.

## KubeVirt

KubeVirt + CDI are **cluster prerequisites**. The Helm chart generates VM/DataVolume YAML; it does not install KubeVirt.

See [examples/kubevirt/migrated-vm.yaml](examples/kubevirt/migrated-vm.yaml).

## Legacy worker DaemonSet

The file-based worker in [`k8s/`](../k8s/) remains available for development. Production deployments should use the Helm chart with Redis transport.
