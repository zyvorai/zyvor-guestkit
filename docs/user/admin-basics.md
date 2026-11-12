# Admin Basics (GuestKit)

## Ports / access

| Port / surface | Service |
|----------------|---------|
| **CLI / TUI** | `guestkit` / `guestctl` on the operator host |
| **8088** | Optional zyvor-ui / Image Vault (GHCR compose eval) |
| **8765** | Guest agent proxy (`guestkit agent-proxy`) when used |
| KubeVirt API | zyvor-api guest / boot-inspect routes when integrated |

Eval web (optional): `http://<host>:8088` — change default admin password immediately. Never publish lab IPs in user docs.

## Auth

- Local CLI needs disk/image access (often root for NBD/loop).
- SAML/OIDC on zyvor-api when the web/API surface is enabled.
- Agent channel: virtio-serial / QGA after inject.

## Install sketch

```bash
curl -fsSL -O https://github.com/zyvorai/zyvor-guestkit/releases/download/v1.2.11/guestkit-1.2.11-linux-amd64.tar.gz
# crates.io `guestkit` is still 0.3.2 — do not cargo install it for this release
# releases ship tar.gz only (no RPM or COPR)
tar xzf guestkit-1.2.11-linux-amd64.tar.gz && cd guestkit-1.2.11-linux-amd64 && ./install.sh   # then run ./guestkit, or add this folder to PATH
export PATH="$PWD:$PATH"
qemu-img --version
guestkit version
guestkit doctor /path/to/disk.qcow2 --target kvm --explain
```

Optional UI: `docker compose -f deploy/docker-compose.ghcr.yml up -d` → open `:8088`.

Host needs: Linux + `qemu-utils` (qemu-img, qemu-nbd). For NBD: `modprobe nbd max_part=16`.

## Related

- [Getting Started](getting-started.md)
- [Troubleshooting](pages/support/troubleshooting.md)
