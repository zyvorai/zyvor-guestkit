# AI code review (guestkit)

_Generated 2026-10-20 by claude-opus-5-5 from a snapshot of tracked sources; verify before acting._

# GuestKit code review

## Summary

GuestKit is a Rust toolkit for offline VM disk inspection. It scores boot readiness, generates and applies fix plans, and emits signed "Cutover Passports", using NBD or loop mounts instead of a libguestfs appliance. The repo also contains:

- a composite GitHub Action (`action.yml`);
- PyO3 bindings (`zyvor-guestkit`);
- an in-guest agent (`zyvor-guest-agent`);
- a job-spec and worker crate;
- an axum/Kubernetes API (`zyvor-api`) with a Helm chart, web UI and Docusaurus docs.

## Bugs and correctness risks

**Agent framing mismatch**
- `src/agent/mod.rs::ping_agent_socket` uses length-prefixed `write_frame`/`read_frame`.
- `src/agent/agent_call.rs` says the libvirt/KubeVirt agent socket is newline-delimited, "not length-prefixed", and uses `write_line`/`read_line`.
- One of the two is wrong. The ping likely always returns `false` against a real QGA socket.

**Fix plans can silently drop operations** (`src/cli/plan/topo_sort.rs`)
- Operations in a dependency cycle are dropped without any error.
- So are operations whose `depends_on` names a non-existent id, because their in-degree never reaches 0.
- Independent operations come out in `HashMap` iteration order, so apply order is nondeterministic.
- For a mutating plan applier this should return an error on cycles or unknown deps, and seed the queue in plan order.

**`fs_frozen` is effectively always false** (`src/evidence/collectors/snapshot_live.rs`)
- It checks whether `/proc/mounts` contains the string `"fsfreeze"`. Frozen state is never shown there.
- It also runs `fsfreeze --help` just to probe for the binary.

**`virtio_net_count` is likely always 0** (`src/collectors/hardware.rs`)
- It counts `/sys/class/net` entries prefixed `virtio`, but interfaces are named `eth*`/`ens*`.
- It should check `/sys/class/net/*/device/driver` instead.

**`action.yml` gate step**
- It runs under `set -uo pipefail` without `-e`. A `doctor` failure inside `| tee` and a failed `passport emit` are not caught directly; the job only fails later, with a less clear error, at `verify`.
- `handoff`, `rescue` and `sbom` run only after `verify` passes. On a failing gate, which is exactly when a rescue plan is most useful, they never run.
- With `require-signature: true` and an empty `trust-keys`, the step passes `--trust-keys ""` instead of failing early with a clear message.
- The score is parsed by `grep -m1 '"score"'` on the JSON, which is fragile if any nested object also has a `score` key.

**`build_python.sh` cannot find the wheel**
- It globs `target/wheels/guestkit-*.whl`, but the distribution is `zyvor-guestkit`, so maturin produces `zyvor_guestkit-*.whl`.
- Under `set -e`, release-mode install fails.

**`install.sh` systemd unit**
- `User=nobody` cannot do NBD or loop mounts.
- `Group=nobody` does not exist on Debian/Ubuntu (`nogroup`).
- `ReadWritePaths=/tmp/guestkit-worker` fails namespace setup if the path does not exist, and is pointless together with `PrivateTmp=true`.

**Helm DSN** (`deploy/helm/zyvor/templates/secrets.yaml`)
- `database-url` interpolates the password into a URL without escaping.
- Passwords containing `@`, `:` or `/` break the DSN.

**`k8s/kustomization.yaml` (likely)**
- `commonLabels: app: guestkit` rewrites the `app` label on pods and selectors.
- The README's `kubectl ... -l app=guestkit-worker` commands would then match nothing. I have not seen the DaemonSet, so this is unconfirmed.
- `commonLabels` is also deprecated.

**Schema drift**
- `deploy/migrations/001_init.sql` lacks the `auth_settings` table that `crates/zyvor-api/src/db.rs` creates.
- There are two sources of truth for the schema.

**Disk shorthand** (`src/cli/invocation.rs`)
- `DISK_EXTENSIONS` omits `vhdx` and `qed`, although the TUI's `IMAGE_EXTS` includes them.
- Any existing file named like a subcommand (e.g. `./commands`) gets rewritten to `inspect`.

**Minor**
- `test_colors_demo.sh` hard-codes `v0.3.0` and the old `hypersdk` URL, and suppresses `rustc` errors.
- `docker-compose.yml` builds from the root `Dockerfile`, which `deploy/README.md` calls deprecated.
- `src/agent/rdp.rs` spawns `true` for no reason.
- `exec_sync` maps signal termination to exit code 1.

## Security concerns

**Script injection in `action.yml`**
- `${{ inputs.rescue }}`, `${{ inputs.rescue-user }}` and `${{ inputs.handoff }}` are interpolated directly into `run:`.
- Pass them through `env:`, as is already done for the other inputs.

**Signing seed exposed in process args**
- `--sign-key "${GK_SIGN_KEY}"` puts the Ed25519 seed on the command line under `sudo`, where it is visible in `ps` and possibly in sudo logs.
- Use a file or stdin instead.

**Fail-open authorization in `zyvor-api`**
- Auth is off by default (`deploy/README.md`).
- When auth is off, `require_guest_remediation_requester` allows everything.
- When auth is on, `rbac::resolve_role` gives every authenticated identity `operator` if `default_role` is empty, which includes remediation rights.
- The default role should be `viewer`.

**Arbitrary command execution in the agent**
- `src/agent/exec.rs::exec_sync` runs any `path` and args with no timeout or allowlist.
- I could not confirm that `policy.rs` gates every caller.

**Broad cluster privileges** (`zyvor-api-rbac.yaml`)
- The ClusterRole grants cluster-wide `pods/exec create`, VM `delete`, and PVC create.
- The worker Deployment combines `hostPID: true`, a `/dev` hostPath and `privileged`.
- `docker-compose.yml` uses `privileged`, `/dev:/dev`, `network_mode: host` and `pid: host`.

**Weak in-cluster defaults**
- The NetworkPolicy allows ingress from every namespace.
- The Redis password is passed as a container `command` argument.
- The `vmtools-reconcile` CronJob calls the API unauthenticated over HTTP.

**Insecure temp files**
- `install.sh` and `test_colors_demo.sh` write to predictable `/tmp` paths and then `sudo cp` from them, which allows symlink or race attacks.

**Supply chain**
- The Action defaults to `version: latest`, and the docs use `@main`.
- Checksums are fetched from the same release, so there is no signature verification.

**`SECURITY.md` problems**
- It routes reports to a personal Gmail address.
- It claims "No unsafe code in core library" while the code uses `libc` and the eBPF crate uses `unsafe`.
- It cites `0.2.x` patch policy.
- It admits LUKS keys are passed on the command line.

## Test and CI gaps

**Crates not covered by the root workspace**
- `crates/guestkit-worker` and `crates/zyvor-api` declare their own `[workspace]` and are not root members, so a root `cargo test` does not cover them.
- `ci.yml` is not shown, so I cannot confirm whether CI covers them separately.

**Action demo does not test PR code**
- `passport-gate-demo.yml` installs the latest *released* binary, not the PR's build, and uses `fail-below: '0'`.
- It verifies the Action's plumbing only.

**Images published without test gating**
- `publish-zyvor-images.yml` pushes GHCR images on every `main` push with no test or build-verification dependency.

**Missing or empty tests**
- None visible for `topo_sort` (cycles, unknown deps).
- None visible for the Helm template rendering (`helm template`/`kubeconform`).
- None visible for the injection-prone Action inputs.
- `src/guestfs/base64_ops.rs::test_base64_ops_api_exists` asserts nothing.

**Live NBD paths skipped on hosted runners**
- `scripts/test-nbd-concurrent.sh` and the cloud-source tests skip their live paths, so concurrency fixes noted in the CHANGELOG (fluxvm#104) are only unit-tested.

## Documentation gaps

**License contradictions**
- `docs/client-presentations/01-guestkit-overview.html` says "LGPL-3.0-or-later guest layer", which contradicts Apache-2.0.
- `copy-zyvor-legal-to-bundle.sh` mentions LGPL, proprietary licensing and "PacketWolf" acceptance.

**Removed acceptance gate still documented**
- CHANGELOG 1.2.3 says the "distribution acceptance gate" was removed.
- `integration/README.md` and `docs/h2kvm-at-a-glance.md` still require `GUESTKIT_ZYVOR_ACCEPT=1`.

**`examples/python/README.md`**
- It uses `--features python`; the actual feature is `python-bindings`.
- It suggests installing `python3-guestfs` (libguestfs) for a missing module.
- It links `GuestKit.org`.
- `build_python.sh` references `examples/python/test_bindings.py`, which is not listed.

**Version and path drift**
- Helm chart is `1.0.1` while the app is `1.2.5`.
- `docs/documentation-map.md` and `docs/development/publishing.md` point to `docs/development/CHANGELOG.md`, but the file is the root `CHANGELOG.md`.

**Inaccurate or broken README snippets**
- `examples/worker-jobs/README.md` shows JSON with `//` comments, which is invalid JSON.
- `crates/guestkit-worker/README.md` says REST and queue transports are "future", but `transport/{http,redis}.rs` exist and Helm uses Redis.
- The README quick start downloads the tarball but never extracts it or verifies the checksum.

**Boilerplate user guides**
- `docs/user/pages/*` are templated stubs ("Doctor — Assurance surface.") with little real content.

## Top 5 recommended actions

1. **Harden `action.yml`.** Move all `${{ inputs.* }}` into `env:`. Pass the sign key via a temp file or stdin. Use `set -e` with explicit `|| true` where intended. Validate `trust-keys` when signatures are required. Run rescue and SBOM even when the g
