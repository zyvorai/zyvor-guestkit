# Copilot instructions for guestkit

Purpose: quick, actionable guidance for AI-assisted coding sessions in this repository.

Build, test, and lint
- Build (dev): cargo build
- Build (release): cargo build --release
- Cross / musl: cargo build --release --target x86_64-unknown-linux-musl
- Install locally: cargo install --path .
- Build examples: cargo build --examples
- Build docs: cargo doc --no-deps --all-features

Rust tests
- Run all tests: cargo test
- Run a single test by name (pattern): cargo test <test_name>
  Example: cargo test test_partition_parsing
- Run a single test exact name: cargo test -- --test-threads=1 <filter>
  (Usually: cargo test <fn_name> -- --nocapture)
- CI runs: cargo test --verbose

Python (PyO3) tests
- Build the Python extension first: maturin develop --release --features python-bindings
- Run Python tests: pytest tests/ or pytest integration/tests/test_integration.py::test_version_command

Linting & formatting
- Formatting check (CI): cargo fmt -- --check
- Apply formatting: cargo fmt
- Linting (CI): cargo clippy --all-targets --all-features -- -D warnings

High-level architecture (big picture)
- Purpose: offline VM disk inspection (qcow2, vmdk, raw) with reporting, TUI, REPL, and optional Python bindings.
- Layers (top → bottom):
  1. Applications / CLI / TUI / Python bindings (entrypoints in src/cli/, src/python.rs)
  2. High-level Rust API (converters, guest detection, pipelines)
  3. Pure-Rust disk layer (src/disk/): DiskReader, partition parsing, filesystem detection
  4. Core utilities: error types, retry logic, types
  5. Optional external tools: qemu-img / qemu-nbd used only for conversion/fallback
- Key modules to consult:
  - src/cli/       — commands, profiles, exporters, TUI, interactive shell
  - src/disk/      — reader, partition, filesystem, loop_device, nbd fallback
  - src/detectors/ — guest OS detection logic
  - src/converters — qemu-img wrapper for format conversion
  - src/python.rs  — PyO3 bindings and Python integration surface
  - docs/architecture/overview.md — more detailed design notes

Key conventions and repository-specific patterns
- No C dependencies: core disk parsing is implemented in pure Rust; qemu-img/qemu-nbd are optional external tools used only for conversion or advanced formats.
- Device strategy: LoopDevice is preferred (fast path for RAW/IMG); NBD (qemu-nbd) used as fallback for QCOW2/VMDK; this strategy is implemented in Guestfs::launch().
- Cargo features:
  - default = ["guest-inspect", "journal-native"]
  - python-bindings (enable PyO3 / maturin)
  - ai (optional rig-core + reqwest)
  Build with features where relevant: cargo build --features python-bindings
- Tests:
  - Rust unit tests live inline in modules (use cargo test)
  - Python integration/tests under tests/ and integration/tests/ (use maturin then pytest)
- CI expectations:
  - Formatting (rustfmt) and clippy warnings are enforced as errors in CI. Keep changes clean: run cargo fmt and cargo clippy locally before opening PRs.
  - Examples, docs and release artifacts are built in CI; see .github/workflows/ci.yml for exact steps.
- Error handling: use the crate Result<T> alias and the central Error enum (thiserror) — follow existing patterns in src/core and modules.
- Minimal unsafe: unsafe blocks are rare and audited; assume disk byte reads and memmap2 use are the main areas.

Where to look first
- README.md (quick start & examples)
- docs/architecture/overview.md (design + module map)
- src/disk/ and src/detectors/ for the core inspection logic
- src/cli/ for how CLI commands wire into the API

Notes for Copilot sessions
- Suggest concrete edits: add tiny, focused changes (tests, docs, clippy fixes). Broad redesigns should be broken into multiple PR-sized steps.
- When changing behavior touching disk handling, run unit tests and, where applicable, the Python integration tests after rebuilding the Python extension.
- Respect CI: run cargo fmt and cargo clippy locally with the CI flags to avoid churn.

Reference files
- .github/workflows/ci.yml — CI steps (formatting, clippy, build, tests, docs)
- Cargo.toml — features, dev-dependencies and targets
- docs/architecture/overview.md — expanded architecture

Releasing & publishing

Publishing crate to crates.io
- Prepare:
  - Bump version in Cargo.toml and optionally update CHANGELOG.md and release notes.
  - Ensure repo is clean, all tests pass, and cargo fmt/clippy run without warnings.
- Verify package contents: cargo package --allow-dirty --no-verify (recommended to run without --allow-dirty on CI)
- Dry run: cargo publish --dry-run
- Publish: cargo publish
  - Requires a crates.io API token configured via `cargo login <TOKEN>` or CARGO_REGISTRY_TOKEN env var.
- Tag & release: create an annotated git tag and push tags; create a GitHub Release with notes.
  Commands (example):
    git tag -s vX.Y.Z -m "guestkit vX.Y.Z" && git push && git push --tags
    gh release create vX.Y.Z target/x86_64-unknown-linux-gnu/release/guestkit -t "guestkit vX.Y.Z" -n "Release notes"

Publishing Python wheels to PyPI (PyO3)
- Build and test locally:
  - maturin build --release --features python-bindings
  - pip install --force-reinstall target/wheels/guestkit-*.whl && pytest tests/
- Publish to PyPI: maturin publish -u __token__ -p "$PYPI_API_TOKEN"
  - Provide PYPI_API_TOKEN via secrets (recommended) or pass credentials interactively.
  - Alternative: build wheel then upload via twine: twine upload target/wheels/*

GitHub Releases (binaries & artifacts)
- `.github/workflows/release.yml` already automates this end to end on a tag push: it builds/uploads binary artifacts per target, creates the GitHub Release via `softprops/action-gh-release@v2`, and runs `cargo publish`. Pushing an annotated tag is normally all that's needed.
- To publish manually instead: download artifacts from CI or build locally for each target, then `gh release create vX.Y.Z path/to/artifact -t "vX.Y.Z" -n "Notes"`.

Release checklist (quick)
- Run cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
- cargo test && cargo doc --no-deps
- cargo package && cargo publish --dry-run
- Create signed git tag and push
- Create GitHub Release (attach CI artifacts)
- For Python: maturin build, test wheel, maturin publish

Automation
- `.github/workflows/release.yml` is the fully automated tag→release pipeline described above — it already exists, gated to trusted maintainers via repo secrets.
