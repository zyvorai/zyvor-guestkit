%global debug_package %{nil}

Name:           guestkit
Version:        1.2.11
Release:        1%{?dist}
Summary:        Pure-Rust VM disk inspection and manipulation toolkit

License:        Apache-2.0
URL:            https://github.com/hypersdk/guestkit
Source0:        %{name}-%{version}.tar.gz

# Rust/Cargo requirements
BuildRequires:  rust >= 1.70
BuildRequires:  cargo
BuildRequires:  gcc
BuildRequires:  make

# System libraries
BuildRequires:  pkgconfig
BuildRequires:  systemd-devel
BuildRequires:  openssl-devel
BuildRequires:  hivex-devel

# Python bindings (optional — off by default; enable with --with python)
%bcond_with python
%if %{with python}
BuildRequires:  python3-devel
BuildRequires:  python3-setuptools
BuildRequires:  python3-pip
BuildRequires:  python3-maturin
%endif

# Runtime dependencies
Requires:       qemu-img
Requires:       nbd
Requires:       util-linux

# Optional dependencies for full functionality
Recommends:     lvm2
Recommends:     mdadm
Recommends:     cryptsetup
Recommends:     device-mapper

# Architecture restrictions (Rust availability)
ExclusiveArch:  x86_64 aarch64 ppc64le s390x

%description
GuestKit is a production-ready toolkit for VM disk inspection and manipulation
with beautiful emoji-enhanced CLI output and an interactive TUI dashboard.
Built in pure Rust for safety and performance, it inspects VM disks in seconds
and integrates cleanly with hyper2kvm for migration workflows.

Features:
- Pure Rust implementation for memory safety and performance
- Interactive TUI dashboard with visual analytics and fuzzy navigation
- Multi-format support: QCOW2, VMDK, VDI, VHD/VHDX, RAW/IMG/ISO
- Security, compliance, hardening, and performance analysis profiles
- Export to JSON, YAML, HTML, PDF formats
- Python bindings for automation (PyO3)
- Interactive REPL shell with 20+ commands
- Batch processing with parallel inspection
- Zero-trust approach - read-only by default

%if %{with python}
%package -n python3-%{name}
Summary:        Python 3 bindings for %{name}
Requires:       %{name}%{?_isa} = %{version}-%{release}

%description -n python3-%{name}
Python 3 bindings for GuestKit, providing native Python API for VM disk
inspection and manipulation.
%endif

%package devel
Summary:        Development files for %{name}
Requires:       %{name}%{?_isa} = %{version}-%{release}

%description devel
Development files, examples, and documentation for %{name}.

%prep
%autosetup -n %{name}-%{version}

%build
# Set Rust optimization flags
export CARGO_TARGET_DIR=target
export RUSTFLAGS="%{?rustflags}"

# Build Rust binary with release profile
cargo build --release --locked --all-features

%if %{with python}
# Build Python bindings
export PYO3_PYTHON=%{__python3}
maturin build --release --strip
%endif

%install
# Install Rust binary
install -Dm755 target/release/guestkit %{buildroot}%{_bindir}/guestkit

%if %{with python}
# Install Python bindings
%{__python3} -m pip install --root %{buildroot} --prefix %{_prefix} --no-deps target/wheels/*.whl
%endif

# Install documentation
install -dm755 %{buildroot}%{_docdir}/%{name}
install -Dm644 README.md %{buildroot}%{_docdir}/%{name}/README.md
install -Dm644 docs/development/CHANGELOG.md %{buildroot}%{_docdir}/%{name}/CHANGELOG.md
install -Dm644 docs/development/CONTRIBUTING.md %{buildroot}%{_docdir}/%{name}/CONTRIBUTING.md
install -Dm644 SECURITY.md %{buildroot}%{_docdir}/%{name}/SECURITY.md

# Install docs directory
cp -r docs %{buildroot}%{_docdir}/%{name}/

# Install examples
install -dm755 %{buildroot}%{_docdir}/%{name}/examples
cp -r examples/* %{buildroot}%{_docdir}/%{name}/examples/

# Install license
install -Dm644 LICENSE %{buildroot}%{_licensedir}/%{name}/LICENSE

# Install bash completion (if available)
# install -Dm644 completions/guestkit.bash %{buildroot}%{_datadir}/bash-completion/completions/guestkit

# Install zsh completion (if available)
# install -Dm644 completions/_guestkit %{buildroot}%{_datadir}/zsh/site-functions/_guestkit

# Install fish completion (if available)
# install -Dm644 completions/guestkit.fish %{buildroot}%{_datadir}/fish/vendor_completions.d/guestkit.fish

%check
# Run basic binary check
%{buildroot}%{_bindir}/guestkit --version

# Run Rust tests (may require test fixtures)
# cargo test --release --locked

%if %{with python}
# Test Python bindings
# %{__python3} -c "import guestkit; print(guestkit.__version__)"
%endif

%files
%license LICENSE
%{_bindir}/guestkit
%dir %{_docdir}/%{name}
%{_docdir}/%{name}/README.md
%{_docdir}/%{name}/CHANGELOG.md
%{_docdir}/%{name}/CONTRIBUTING.md
%{_docdir}/%{name}/SECURITY.md
%{_docdir}/%{name}/docs/
# %{_datadir}/bash-completion/completions/guestkit
# %{_datadir}/zsh/site-functions/_guestkit
# %{_datadir}/fish/vendor_completions.d/guestkit.fish

%if %{with python}
%files -n python3-%{name}
%license LICENSE
%{python3_sitearch}/%{name}/
%{python3_sitearch}/%{name}-%{version}.dist-info/
%endif

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
- Initial RPM package for Fedora/RHEL
- Interactive TUI dashboard with fuzzy jump navigation (Ctrl+P)
- Security, compliance, hardening, performance profiles
- Export to JSON, YAML, HTML, PDF formats
- Interactive shell with 20+ commands for disk exploration
- Python bindings via PyO3 (optional)
- Support for QCOW2, VMDK, VDI, VHD/VHDX, RAW, IMG, ISO formats
- LVM, RAID, fstab inspection
- Network, services, databases, web servers detection
- Batch processing with parallel inspection
- Caching system for performance
- Clean build with zero compiler warnings
- Reorganized documentation structure
- Removed legacy-appliance-tooling references (pure Rust implementation)

* Mon Jan 26 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.3.0-1
- Add comprehensive TUI dashboard with multiple views
- Add security analysis profiles (5 types)
- Add enhanced inspection APIs
- Improve documentation with examples
- Add export formats (JSON, YAML, HTML, PDF)

* Sat Jan 24 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.2.0-1
- Add Python bindings via PyO3
- Add batch processing support with parallelization
- Add inspection caching system
- Improve error handling and reporting
- Add retry mechanisms for operations

* Fri Jan 23 2026 ZyvorAI Labs Private Limited <ssahani@zyvor.dev> - 0.1.0-1
- Initial release
- Basic VM disk inspection functionality
- Support for multiple disk formats (QCOW2, VMDK, etc.)
- CLI interface with emoji-enhanced output
- Read-only operations by default
