# MCP server

`guestkit mcp-serve` is a stdio [Model Context Protocol](https://modelcontextprotocol.io)
server for **one** VM disk image. It mounts and inspects the image once at
start-up, then serves six **read-only** evidence tools to an MCP client (Claude
Desktop or any stdio MCP host). It does not modify the disk.

## From source

The server is behind the `mcp` Cargo feature (which pulls in `ai`):

```bash
cargo build --release --features mcp
guestkit mcp-serve /vms/web01.qcow2 --target kvm
```

The host needs the same loop/NBD access as the rest of the CLI (`losetup`,
`qemu-nbd`; root in practice). The process is meant to be launched by an MCP
client, not run interactively.

## From the MCP Registry (OCI)

The server is declared for the MCP Registry as `io.github.zyvorai/guestkit` and
ships as the multi-arch, cosign-signed image `ghcr.io/zyvorai/guestkit-mcp`,
built from `Dockerfile.mcp` (the top-level `Dockerfile` CLI image is separate).
Entries are published from `server.json` by the `publish-mcp` job in
`.github/workflows/release.yml` on every `v*` tag; `scripts/check-mcp-registry.sh`
keeps `server.json`, the image label and `Cargo.toml` in step.

To check whether an entry exists, query the registry:

<https://registry.modelcontextprotocol.io/v0/servers?search=io.github.zyvorai/guestkit>

The server serves one image, so the disk is supplied at launch: bind-mount its
directory read-only and pass the in-container path as the argument. Client
configuration for any stdio MCP client:

```json
{
  "mcpServers": {
    "guestkit": {
      "command": "docker",
      "args": ["run", "-i", "--rm", "--user", "0", "--privileged",
               "-v", "/srv/vms:/vms:ro",
               "ghcr.io/zyvorai/guestkit-mcp:<version>",
               "/vms/web01.qcow2", "--target", "kvm"]
    }
  }
}
```

Replace `<version>` with a published tag of the image.

### Privileges

The image runs as the unprivileged `nonroot` user (uid 65532), but opening a
disk needs loop/NBD access, which needs root and device access. In practice add
`--user 0 --privileged` (or `--cap-add SYS_ADMIN` plus the loop/NBD devices) to
the `docker run` arguments, as above. Only mount disk images you trust into a
privileged container.

See also [Docker deployment](DOCKER.md#install-from-the-mcp-registry-oci) for the
same section alongside the web stack and CLI images.
