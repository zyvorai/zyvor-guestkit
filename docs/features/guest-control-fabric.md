# Zyvor Guest Control Fabric

Zyvor does **not** depend on guest networking for VM intelligence. Guest control is **transport-independent**: the API selects the best available path per VM and per operation.

## Transport ladder (priority order)

| Tier | Transport | When used |
|------|-----------|-----------|
| 1 | `virtio-serial` | Zyvor agent daemon + QGA channel |
| 2 | `qga-exec` | QGA up → `guest-exec` → `zyvor-guest-agent` JSON-RPC |
| 3 | `qga-builtin` | QGA `guest-ping`, `guest-file-*`, freeze/thaw |
| 4 | `in-guest-socket` | Local agent socket probe via QGA exec |
| 5 | `https-push` | Redis-cached push report or HTTP agent proxy |
| 6 | `offline-disk` | Halted VM → GuestKit repair/inspect job on root PVC |
| 7 | `console-only` | Running VM, no QGA — structured error + recommendations |

Each pull records `attempts: [{ tier, ok, latencyMs, error }]` for Agent Doctor and debugging.

## Control states (UI)

| State | Meaning |
|-------|---------|
| `full_agent` | Agent daemon running with network or push |
| `airgap_live` | Agent + QGA, no guest network — host-mediated pull |
| `qga_only` | QGA connected, Zyvor agent missing |
| `disk_only` | VM stopped, offline repair possible |
| `console_only` | VM running, no QGA |
| `blind_vm` | No live or offline path |

## Capability contract

`GET /api/v1/kubevirt/vms/{ns}/{name}/guest/capabilities` returns:

```json
{
  "ok": true,
  "transport": "qga-exec",
  "controlState": "airgap_live",
  "capabilities": {
    "network": false,
    "qga": true,
    "zyvorAgent": true,
    "supports": { "evidence": true, "exec": true, "freeze": true, "pushTelemetry": false }
  },
  "warnings": ["guest network unavailable"],
  "recommendedActions": ["install_agent_via_qga"]
}
```

## API routes

| Method | Path | Purpose |
|--------|------|---------|
| GET | `.../guest/status` | Control state + probes |
| GET | `.../guest/capabilities` | Capability contract |
| GET/POST | `.../guest/doctor` | Agent Doctor tree; POST runs live `guestkit.doctor` |
| GET | `.../guest/readiness` | Migration readiness score 0–100 |
| POST | `.../guest/install-agent` | Strategy-aware install |
| POST | `.../guest/repair-plan` | Offline repair for halted VM |
| POST | `.../guest/file/read` | QGA file read |
| POST | `.../guest/file/write` | QGA file write (airgap bootstrap) |
| POST | `/kubevirt/guest/poll-reconcile` | Poll AirgapLive VMs without push |
| GET | `/kubevirt/guest/poll-telemetry` | Fleet airgap poll rollup |
| GET | `.../guest/poll-telemetry` | Per-VM latest poll sample (latency + attempts) |

All guest routes return a **GuestControlEnvelope**: `ok`, `transport`, `networkRequired`, `controlState`, `capabilities`, `warnings`, `recommendedActions`, `data`.

## Airgap install (QGA file bootstrap)

When QGA is up but guest network is down:

1. API fetches `zyvor-vm-tools-linux-amd64.tar.gz` from the cluster bundle URL
2. Chunks are written to `/tmp/zyvor-vm-tools.tar.gz` via `guest-file-open/write/close`
3. `guest-exec` unpacks and enables `zyvor-guest-agent.service` — **no curl in guest**

Trigger: `POST .../guest/install-agent` with `{ "strategy": "auto" }` (auto-selects `qga_file_bootstrap`).

## Host-mediated polling

Background worker (`GUEST_AIRGAP_POLL_ENABLED`, default on) polls VMs in `airgap_live` without push heartbeat every 30s (`GUEST_AIRGAP_POLL_INTERVAL_SECS`).

Each cycle stores:

| Redis key | Contents |
|-----------|----------|
| `guest-agent:vm-poll:{ns}:{name}` | Per-VM sample: method latency, transport attempts, probe ladder |
| `guest-agent:poll-fleet` | Fleet rollup: scanned/polled/skipped, avg latency, error list, samples |

API:

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/kubevirt/guest/poll-reconcile` | Run one reconcile now |
| GET | `/kubevirt/guest/poll-telemetry` | Fleet rollup |
| GET | `/kubevirt/vms/{ns}/{name}/guest/poll-telemetry` | Latest VM poll sample |

`GET .../guest/status` includes `lastPoll` + `telemetryMode` (`pull_via_virt_launcher` / `push` / `none`).

UI label: **Telemetry mode: Pull via virt-launcher**.

## Security

`GuestActionPolicy` CRD extensions:

- `execAllowlist`, `fileReadAllowlist`, `fileWriteAllowlist`
- `freezeAllowed`, `maxExecOutputBytes`
- JIT approval for exec, file write, install-agent

Audit events include `transport` and `networkRequired`.

## Related docs

- [Guest agent](guest-agent.md)
- [Zeus VM Tools](zeus-vm-tools.md)
- [KubeVirt integration](kubevirt-integration.md)
