Name:           guestkit
Version:        1.2.11
Release:        1%{?dist}
Summary:        Pure-Rust VM disk inspection and manipulation toolkit

License:        Apache-2.0
URL:            https://github.com/hypersdk/guestkit
Source0:        %{name}-%{version}.tar.gz

# Build dependencies
BuildRequires:  rust >= 1.70
BuildRequires:  cargo
BuildRequires:  gcc
BuildRequires:  pkgconfig
BuildRequires:  systemd-devel

# Runtime dependencies
Requires:       qemu-img
Requires:       nbd
Requires:       util-linux

# Optional dependencies for full functionality
Recommends:     lvm2
Recommends:     mdadm
Recommends:     cryptsetup

%description
GuestKit is a production-ready toolkit for VM disk inspection and manipulation
with beautiful emoji-enhanced CLI output and an interactive TUI dashboard.
Built in pure Rust for safety and performance, it inspects VM disks in seconds
and integrates cleanly with hyper2kvm for migration workflows.

Features:
- Pure Rust implementation for memory safety and performance
- Interactive TUI dashboard with visual analytics
- Multi-format support: QCOW2, VMDK, VDI, VHD/VHDX, RAW/IMG/ISO
- Security profiles for compliance and hardening analysis
- Export to JSON, YAML, HTML, PDF formats
- Python bindings for automation
- Interactive REPL shell for disk exploration
- Batch processing with parallel inspection

%package devel
Summary:        Development files for %{name}
Requires:       %{name}%{?_isa} = %{version}-%{release}

%description devel
Development files and headers for %{name}.

%prep
%autosetup -n %{name}-%{version}

%build
# Build with release profile
export CARGO_TARGET_DIR=target
cargo build --release --locked

%install
# Install binary
install -Dm755 target/release/guestkit %{buildroot}%{_bindir}/guestkit

# Install documentation
install -Dm644 README.md %{buildroot}%{_docdir}/%{name}/README.md
install -Dm644 docs/development/CHANGELOG.md %{buildroot}%{_docdir}/%{name}/CHANGELOG.md
install -Dm644 docs/development/CONTRIBUTING.md %{buildroot}%{_docdir}/%{name}/CONTRIBUTING.md
install -Dm644 SECURITY.md %{buildroot}%{_docdir}/%{name}/SECURITY.md

# Install docs directory
cp -r docs %{buildroot}%{_docdir}/%{name}/

# Install examples
mkdir -p %{buildroot}%{_docdir}/%{name}/examples
cp -r examples/* %{buildroot}%{_docdir}/%{name}/examples/

# Install license
install -Dm644 LICENSE %{buildroot}%{_licensedir}/%{name}/LICENSE

# Install man page (if we create one)
# install -Dm644 doc/guestkit.1 %{buildroot}%{_mandir}/man1/guestkit.1

%check
# Run tests (currently skipped due to test requirements)
# cargo test --release --locked

%files
%license LICENSE
%{_bindir}/guestkit
%{_docdir}/%{name}/
%exclude %{_docdir}/%{name}/examples/

%files devel
%{_docdir}/%{name}/examples/

%changelog
* Fri Nov 13 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 1.2.11-1
- Version brought in line with Cargo.toml (was stale at 1.0.1); no packaging change

* Sat Aug 15 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 1.0.1-1
- Fix guestkit-worker's Docker release build: its pinned `guestkit = "0.3.3"`
  path dependency rejected the workspace's own 1.0.0, breaking GHCR image
  publishing for every release past the 0.x line

* Sat Aug 15 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 1.0.0-1
- MCP server for the AI copilot (guestkit mcp-serve), native OpenAI tool-calling, cross-run AI memory
- Fleet dependency-aware migration waves (fleet wave-plan) and scheduled drift monitoring (fleet watch)
- GitHub Action for the Passport CI gate (action.yml)
- k3s E2E and CI reliability fixes; Helm chart lint/template CI coverage

* Sat Aug 08 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.3.21-1
- Fix SEGV in offline Windows password reset (hivex_value_type FFI signature)
- ntfsfix now actually clears the NTFS dirty flag on Windows rescue writes
- Fix Windows cross-compile break from ungated Unix-only guestfs modules

* Fri Aug 07 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.3.20-1
- DevOps runbooks, GitHub Wiki operator cheat sheets
- Windows AES/RC4 SAM NT-hash write for offline password reset
- UEFI-aware fix-grub --force; offline PackageInstall staging + host fetch
- Cutover Passport signed-enterprise workflows (keygen/issuer/trust-keys)

* Thu Aug 06 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.3.19-1
- Cutover Passport (emit/verify CI gate; HyperSDK/hyper2kvm handoff)
- Windows day-0: domain-leave, timezone, static-ip plan profiles

* Thu Aug 06 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.3.18-1
- Day-0 windows-hostname / windows-winrm / hardened linux-ssh plans
- Offline DriverInject via GUESTKIT_VIRTIO_WIN; migrate-repair --virtio-win
- Windows SAM blank rescue; rescue --export-plan; heuristic offline remediations

* Thu Aug 06 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.3.17-1
- Offline Linux SSH rescue + linux-ssh plan profile
- Windows RDP plan profile, plan apply --skip-backup, NTFS ntfsfix before apply
- Windows guest-fsfreeze-freeze/thaw routes to VSS (KubeVirt freeze)

* Wed Jul 29 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.3.15-1
- Sync spec Version with Cargo.toml (was stuck at 0.3.9, breaking RPM builds
  since 0.3.10 — Source0 tarball name never matched the built tarball)
- In-guest Windows agent offline install (service + vioser driver via hivex,
  stock qemu-ga takeover, converted-image driver fix)
- Generic guestkit-rpc QGA passthrough

* Fri Jun 13 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.3.9-1
- Fix CI formatting, integration tests, RPM verify, and PyPI wheel publish

* Fri Jun 12 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.3.8-1
- Fix CI clippy and integration copy test; refresh Cargo.lock

* Fri Jun 12 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.3.7-1
- IronWolf web console theme for GuestKit deploy UI
- Console Copilot API, system status, fleet overview endpoints
- zyvor-guest-agent crate and QGA transport improvements
- Windows forensic EVTX parsing and persistence collectors

* Thu Jun 04 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.3.6-1
- Add in-guest agent mode for live VM assurance over virtio-serial
- Sync RPM spec version with Cargo.toml

* Tue Jan 27 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.3.1-1
- Initial RPM package
- Add interactive TUI dashboard with fuzzy jump navigation
- Add security, compliance, hardening, performance profiles
- Add export to JSON, YAML, HTML, PDF
- Add interactive shell with 20+ commands
- Add Python bindings via PyO3
- Remove legacy-appliance-tooling references
- Reorganize documentation structure
- Fix compiler warnings

* Mon Jan 26 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.3.0-1
- Add comprehensive TUI dashboard
- Add security analysis profiles
- Add enhanced inspection APIs
- Improve documentation

* Sat Jan 24 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.2.0-1
- Add Python bindings
- Add batch processing support
- Add caching system
- Improve error handling

* Fri Jan 23 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.1.0-1
- Initial release
- Basic VM disk inspection
- Support for multiple disk formats
- CLI interface
