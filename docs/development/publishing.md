# Publishing GuestKit

## PyPI (`zyvor-guestkit`)

The Python distribution is **`zyvor-guestkit`**, owned by the [zyvor](https://pypi.org/user/zyvor/) account. Do not publish to `hypersdk-guestkit`. The import name stays `guestkit`.

`PYPI_API_TOKEN` must be an API token from that account (https://pypi.org/manage/account/token/). The first upload creates the project; the old `hypersdk` token cannot.

```bash
pip install zyvor-guestkit
```

CI workflow: **Build and Publish Python Wheels** — requires GitHub secret `PYPI_API_TOKEN`. The wheels are `abi3-py310` (Python 3.10 or later; 3.8 and 3.9 were dropped in PR #84) and the install test runs on 3.10, 3.11, 3.12 and 3.13.

Manual publish after building wheels locally:

```bash
maturin build --release --features python-bindings --out dist
twine upload dist/*
```

## crates.io (`guestkit`)

The Rust crate name stays **`guestkit`**. That crate is owned by the crates.io user `ssahani`. The newest version there is **0.3.2**. `cargo install guestkit` does not install v1.2.11.

`CARGO_TOKEN` is a token for the `zyvorai` account. That account can publish only after `ssahani` adds it and the invite is accepted:

```bash
cargo owner --add zyvorai guestkit
```

`guestkit-agent-protocol` 0.1.0 is already published by `zyvorai`. The main crate depends on it. Until an owner publishes `guestkit` 1.2.11, install the [GitHub Release](https://github.com/zyvorai/zyvor-guestkit/releases/tag/v1.2.11) binaries instead of crates.io.

The Release workflow no longer publishes to crates.io; use the GitHub Release binaries, PyPI (`zyvor-guestkit`) or the GHCR images.

## GitHub release assets

Tag push `v*` runs the [Release workflow](../../.github/workflows/release.yml), which:

- creates the GitHub Release; the notes are the `## [<version>]` section of `docs/development/CHANGELOG.md` (the run fails when that section is missing, so keep it in step with the root `CHANGELOG.md`);
- uploads `guestkit-<version>-linux-amd64.tar.gz` (glibc) and `guestkit-<version>-linux-amd64-musl.tar.gz` (static), each with a `.sha256` file. The tarball holds `guestkit`, a `guestctl` symlink, installer and test scripts and legal files; it does not hold `guestkit-qemu` or `virtctl-guestkit`;
- uploads the Zeus VM Tools guest artifacts: `zyvor-vm-tools-linux-amd64.tar.gz` (+ `.sha256`), `zyvor-vm-tools_<version>_amd64.deb` and `zyvor-vm-tools.iso`. These are separate guest-side packages, not the host CLI;
- publishes the `guestkit-worker`, `zyvor-api` and `zyvor-ui` images to `ghcr.io/zyvorai` (`:v<version>`, `:latest`), then calls the [k3s E2E workflow](../../deploy/README.md#ci-k3s-e2e) (`k3s-e2e.yml` via `workflow_call`, job `k3s-e2e`) as a release smoke test of those images;
- builds and signs `ghcr.io/zyvorai/guestkit-mcp:<version>` and publishes `server.json` to the MCP Registry;
- publishes the Python wheel to PyPI as `zyvor-guestkit`;
- moves the `v1` tag used by the `zyvorai/guestkit` GitHub Action.

**No RPM or COPR.** The `RPM Build` workflow was removed after v1.2.11 (PR #81): releases ship tar.gz only. `guestkit.spec` and `guestkit-full.spec` remain for local builds, see [RPM-BUILD.md](RPM-BUILD.md). Run `python3 scripts/check-doc-versions.py` before tagging: it fails when the install snippets in the docs still name an older release than `Cargo.toml`. The release's version gate (`scripts/check-mcp-registry.sh`) requires `Cargo.toml`, `server.json` and the tag to agree, so bump them together.
