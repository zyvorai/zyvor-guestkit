# GuestKit architecture (v1.2.11)

Offline VM intelligence and migration assurance — **Rust control plane** with host block-device access for mount-heavy operations.

## Positioning (accurate)

| Claim | Reality |
|-------|---------|
| **no appliance daemon** | ✅ No legacy appliance tooling daemon workflow |
| **Pure Rust parsing** | ✅ Partition tables, FS signatures, evidence schema, boot engine, assurance APIs |
| **In-process QCOW2 file read** | Partial — format detection + selective reads; full cluster walk defers to **qemu-nbd** |
| **File access inside guests** | Via **loop devices / qemu-nbd** + host mount (`src/guestfs/`), not in-process ext4/NTFS parsers |
| **Web UI** | ✅ Shipped — `deploy/ui/` (inventory + Assurance + Profiles + Files), GHCR `zyvor-ui` |

**Host dependencies (Linux):** `losetup` (raw, img, iso), `qemu-nbd` (QCOW2, VMDK, VDI, VHD, VHDX), kernel `loop`/`nbd` modules, and `qemu-img` for format conversion and `guestkit img`. Attaching and mounting need root (GuestKit calls `sudo` when it is not root).

## System layers

```text
┌─────────────────────────────────────────────────────────────────┐
│  Surfaces: guestkit · guestctl · guestkit-qemu · Python · UI    │
├─────────────────────────────────────────────────────────────────┤
│  Assurance: evidence → boot score → migrate-plan → fleet/policy │
│             → GuestKitQemuPlan (gated QEMU/VirtIO launch)       │
├─────────────────────────────────────────────────────────────────┤
│  Live QGA: guestkit qga / agent-call (unix socket; no virsh)    │
├─────────────────────────────────────────────────────────────────┤
│  AI (optional): deterministic intel + `--features ai` LLM agent │
├─────────────────────────────────────────────────────────────────┤
│  guestfs façade: Rust orchestration + NBD/loop + mount        │
├─────────────────────────────────────────────────────────────────┤
│  Disk parsers: MBR/GPT, FS magic, registry/hive (Rust)          │
└─────────────────────────────────────────────────────────────────┘

Parallel platform runtime (same repo):
  zyvor-api (Axum) → Redis job queue → guestkit-worker (privileged)
    jobs: inspect · doctor · migrate-plan · passport · repair · profile · explore · convert · agent.*
  PostgreSQL · KubeVirt client · guest-agent mTLS · PacketWolf hooks
  UI: deploy/ui (static) proxies /api → zyvor-api; optional serve-https.py lab TLS
```

## Repository layout

| Path | Role |
|------|------|
| `src/` | Main `guestkit` crate — CLI, guestfs, boot, evidence, assurance, fleet, TUI, qemu |
| `src/agent/` | In-guest agent + host `qga_client` / `guestkit qga` |
| `src/qemu/` | Declarative QEMU/VirtIO config, GuestKit plan bridge, QMP client |
| `src/ai/` | Deterministic intelligence; LLM agent behind `ai` feature |
| `crates/guestkit-job-spec` | Worker job schema |
| `crates/guestkit-worker` | Redis-queue disk inspection daemon |
| `crates/guestkit-agent-protocol` | Guest agent JSON-RPC framing |
| `crates/zyvor-api` | Web API, auth, KubeVirt, QGA socket ladder (`qga_socket`) |
| `crates/zyvor-guest-agent` | In-guest agent binary |
| `deploy/` | Docker Compose, Helm, UI static assets |
| `k8s/` | KubeVirt-oriented manifests |

## Core data flow

```text
Disk image → guestfs mount (ro) → EvidenceSnapshot
                                        │
                    ├─► BootabilityReport (doctor)
                    ├─► MigrationScoreReport (migrate-plan)
                    ├─► Policy evaluation (policy check)
                    ├─► Fleet clusters (fleet analyze)
                    ├─► FixPlan (repair / plan apply)
                    └─► GuestKitQemuPlan → QEMU argv / QMP
```

Evidence is cached under `~/.cache/guestkit/` after successful `doctor` runs.

## Crates and workspace

The root `Cargo.toml` workspace includes the main package, `guestkit-agent-protocol`, `guestkit-job-spec`, `zyvor-guest-agent`, `guestkit-ebpf-common` and `guestkit-ebpf-runtime`; `crates/guestkit-ebpf` (kernel-side programs) is excluded and built by `guestkit-ebpf-runtime/build.rs`. `zyvor-api` and `guestkit-worker` build as sibling crates with their own lockfiles and path dependencies (see each crate `Cargo.toml`).

**Version:** 1.2.11 (package) · **License:** Apache-2.0 · **Owner:** ZyvorAI Labs Private Limited

## Guest-file trust boundary (v1.2.11)

Guest files are reached through a host mount of the attached device, so everything in the image is untrusted input to the host kernel and to GuestKit. The guestfs layer enforces:

- **Mount:** every host `mount` of a guest filesystem adds `nodev,nosuid` (`harden_mount_opts` in `src/guestfs/mount.rs`); mount points containing `..` are rejected.
- **Path resolution:** `resolve_in_root` (`src/guestfs/file_ops.rs`) walks the path component by component, following symlinks as `chroot(root)` would (absolute targets restart at the root, `..` stops at the root, at most 40 symlink hops). File names that merely contain `..` are valid. `glob_expand`, tar/archive and `cpio` operations use the same root and reject escapes.
- **Reads:** `read_file` opens with `O_NONBLOCK`, requires a regular file and caps the size at 1 GiB.
- **External tools:** image paths are absolutised before `losetup`, `qemu-nbd` and `qemu-img`; `grep`, `egrep` and `fgrep` pass the pattern after `-e` and `--`.
- **Shared state:** the NBD allocation lock is a 0600 `O_NOFOLLOW` file in `/run/lock` or the per-user runtime directory (never `/tmp`); the binary cache directory is 0700 and per-user.
- **Worker and API:** job ids are validated as a single safe path component; worker `convert` allowlists formats and refuses to overwrite its source; `zyvor-api` allowlists the stored image extension, removes partial uploads on failure and no longer overwrites an unrelated stored file on storage import.

`guestkit execute` is outside this boundary: it `chroot`s into the guest root on the host and runs the image's own binaries as root.

## Security model (web stack)

- **Eval default:** `AUTH_ENABLED=false` in `deploy/docker-compose.ghcr.yml` — localhost demos only
- **Production:** auth on, `JWT_SECRET`, `AGENT_BOOTSTRAP_TOKEN`, Redis password — see [DOCKER.md](../guides/DOCKER.md#production-checklist)
- **OIDC:** ID tokens verified via JWKS (signature, issuer, audience, expiry)

## Further reading

- [Migration assurance](../features/migration-assurance.md)
- [QEMU / VirtIO runtime](../features/qemu-runtime.md)
- [Dump virsh → GuestKit](../user-guides/virsh-to-guestkit.md)
- [Guest agent](../features/guest-agent.md)
- [KubeVirt integration](../features/kubevirt-integration.md)
- [CE vs Enterprise](../ce-vs-enterprise.md)
- [User stories](../USER_STORIES.md)
