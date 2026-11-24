# Offline image inspection: landscape and ideas

Scope: tools that inspect a VM disk or image **without booting it**, and what
GuestKit can learn from them. Items marked **[U]** were not read from a fetched
page in this session (recalled from general knowledge) and should be verified
before being quoted externally. Everything else was read from the cited page on
2026-11-23.

GuestKit hard boundaries that constrain every idea below: zero boot, zero
agents in the guest, read-only inspection, and honesty about how sure a
diagnosis is (see `README.md` and `SECURITY.md`).

## What the field does

| Tool | What it does offline | Source |
|---|---|---|
| libguestfs `virt-inspector` | Reports OS class, distro, version, architecture, mount points, filesystems (type, label, UUID) and installed applications as XML; supports LUKS unlock. Does not report drivers (separate `virt-drivers`). | https://libguestfs.org/virt-inspector.1.html |
| libguestfs `virt-sysprep` | Not an audit but a catalogue of what makes an image unfit to clone: SSH host keys, machine-id, persistent net rules, logs, bash history, package caches, user accounts, DHCP state, cron spool, rpm db. Most operations run by default; `fs-uuids` is discouraged because it can make a guest unbootable. | https://libguestfs.org/virt-sysprep.1.html |
| Trivy `trivy vm` | Scans a local VM image or an AWS AMI/EBS snapshot for vulnerabilities and secrets (default), misconfigurations and licenses (opt-in), and can emit an SBOM. Experimental; VMDK (streamOptimized) only, MBR/GPT, XFS/ext2-4; no LVM; marketplace AMIs unsupported. | https://trivy.dev/latest/docs/target/vm/ |
| Syft | SBOM generator for container images, filesystems, archives and directories across dozens of package ecosystems; CycloneDX, SPDX and Syft JSON; in-toto attestations. | https://github.com/anchore/syft |
| OpenSCAP `oscap-chroot` / `oscap-vm` | Evaluates SCAP content against a filesystem mounted at an arbitrary path (`OSCAP_PROBE_ROOT`); no agent, no change to the guest. `oscap-podman` needs root. | https://static.open-scap.org/openscap-1.3/oscap_user_manual.html |
| Lynis | Host-based hardening audit (configuration, vulnerable packages, compliance mapping to ISO 27001/PCI-DSS/HIPAA). The fetched page does not say whether it supports chroot or offline use. | https://github.com/CISOfy/lynis |
| Wazuh **[U]** | Agent-based SCA/FIM/vulnerability detection; needs an agent in the guest, so it is not an offline tool. Its SCA policy YAML (checks as data) is the relevant idea. | (not fetched) |
| Amazon Inspector | Agentless EC2 scanning from EBS snapshots: OS and language package vulnerabilities for ext3/ext4/xfs volumes, limited to fewer than 8 volumes and 1200 GB combined. Agent-based mode needs the SSM agent. | https://docs.aws.amazon.com/inspector/latest/user/scanning-ec2.html |
| gitleaks | Regex plus entropy rules, allowlists, baselines, archive scanning, `--redact`. In maintenance mode (author moving to Betterleaks). | https://github.com/gitleaks/gitleaks |
| Packer / EC2 Image Builder validation **[U]** | Validate and test stages run inside a booted builder instance, not offline. | (not fetched) |

Patterns worth noting: (1) everyone that works offline mounts the image and
walks the tree, so confinement of symlinks in hostile images is a shared
problem that few of them document; (2) Trivy and Inspector publish explicit
support limits (filesystems, LVM, volume counts), which is the honest-confidence
behaviour GuestKit should also show; (3) virt-sysprep's operation list is the
de facto checklist of "baked-in identity" hazards.

## What GuestKit already has

`inspect`, `secrets` (v2, entropy and rules, redacted), `cve`/OpenVEX/CSAF,
`inventory` (SPDX/CycloneDX), `sbom-diff`, `forensic-diff`, `doctor` boot
checks over the evidence snapshot (fstab UUID consistency, initramfs presence,
GRUB, EFI, virtio, cloud-init conflicts), `migrate-assess`, `passport`,
`harden` (an apply-oriented fix list, not an analysis).

## What it lacked (gaps)

1. **No rule-based, evidence-citing hardening audit.** `harden` prints a fixed
   change list; nothing reads the effective sshd configuration (Include,
   first-match semantics), sudoers includes, PAM stacks, `login.defs` or account
   databases and says which line is the problem. OpenSCAP and Lynis cover this
   online or via SCAP content; neither is read-only-by-construction over a disk
   image with confidence labels.
2. **Boot checks are thin on references.** Existing checks look at counts and
   flags in the evidence snapshot; they do not verify that a GRUB or BLS entry
   points at files that exist, that each kernel has its initramfs and modules,
   that `root=`/fstab UUIDs resolve to a filesystem actually in the image, or
   that cloud-init and netplan files parse.
3. **No image-to-image diff of audit state** (there are SBOM and passport
   diffs, but not a diff of hardening and boot findings).
4. **No "baked-in identity" report** in the virt-sysprep sense beyond host keys
   (machine-id, DHCP leases, persistent net rules, cloud-init instance state,
   shell history).
5. **Secrets scanning of images is covered** (gitleaks-class), so not a gap.
6. **SBOM export exists** (CycloneDX/SPDX via `inventory`), so not a gap.
7. Partition/filesystem forensics (deleted-file carving, slack space, timeline
   from filesystem metadata) is largely out of scope for a "what is wrong before
   you boot it" product.

## Ranked ideas

| Rank | Idea | Why it fits | Decision |
|---|---|---|---|
| 1 | Offline hardening audit with a rule table (SSH, sudo, PAM, login.defs, accounts), confidence per finding, no secret values | Pure read-only file analysis; directly answers "what is broken" before boot; closes the biggest gap; rules are data and testable on synthetic trees | **Built** (`image-audit`, scope `hardening`) |
| 2 | Boot-readiness reference checks: kernel/initramfs/modules pairing, BLS and GRUB entry targets, `root=`/`resume=`/fstab UUID and LABEL resolution, cloud-init YAML, netplan, MAC-pinned network config | Most real "won't boot after migration" causes are dangling references; checks are static and explainable | **Built** (`image-audit`, scope `boot`) |
| 3 | Machine-readable diff of two audits (new, resolved, severity-changed, verdict, `--fail-on-regression`) | Lets CI compare golden image vs candidate; reuses the existing `diff`-style verdicts | **Built** (`image-audit-diff`) |
| 4 | MCP tool exposing the audit to AI hosts | Same data, read-only, follows existing `mcp-serve` pattern | **Built** (`get_offline_audit`) |
| 5 | virt-sysprep-style "baked-in identity" report (machine-id, DHCP leases, shell history, cloud-init state) | Useful for clone safety; overlaps rank 1/2 findings already emitted (host keys, authorized_keys, instance state, persistent-net) | Partly covered; remaining items deferred |
| 6 | SCAP/OVAL or Wazuh-SCA-style rule packs loaded from YAML | Would let users add rules without a release; needs a stable rule schema first | Deferred: ship the rule table in code first |
| 7 | SBOM export from package databases | Already exists (`inventory -f cyclonedx|spdx`) | **Rejected** (duplicate) |
| 8 | Secrets scan with redaction | Already exists (`secrets` v2) | **Rejected** (duplicate) |
| 9 | Filesystem forensics (carving, slack, timelines) | Different product; large attack surface from hostile filesystems | **Rejected** |
| 10 | Running Lynis/OpenSCAP inside a chroot of the image | Executes guest-supplied code paths (shell, interpreters) against hostile content and breaks "no execution of guest code" | **Rejected** |

## Design consequences adopted

* Findings carry `confidence`: `confirmed` (exact setting read), `likely`
  (depends on distro defaults or on filesystems the offline scan cannot see
  through, such as LVM and LUKS), `heuristic` (cannot be proven offline).
* A `notes` list records every limit that was hit (file over cap, symlink loop,
  `Match` blocks not evaluated, unverifiable device references), so silence
  never means "checked and fine" when it was not.
* Reports never contain secret values: password hashes, private keys and
  authorized key material are counted or named, not copied.
* All reads go through one confined reader (symlinks resolved against the guest
  root, non-regular files refused, size/fan-out/include-depth caps).

## Round 3 (2026-11-24): turning the audit into a workflow

Read from fetched pages on 2026-11-24:

| Source | What it showed | Source URL |
|---|---|---|
| virt-sysprep man page | Full operation list (machine-id, ssh-hostkeys, dhcp-client-state, udev-persistent-net, net-hwaddr, bash-history, cron-spool, passwd-backups, mail-spool, rh-subscription-manager, rhn-systemid, yum-uuid, kerberos-hostkeytab, logfiles, tmp-files, crash-data, package-manager-cache, ...) and the shared random-seed hazard | https://libguestfs.org/virt-sysprep.1.html |
| Trivy filtering docs | `.trivyignore.yaml` entries have `id`, `paths`, `purls`, `expired_at`, `statement`; `--show-suppressed` lists what was suppressed; the YAML file is experimental. The page does not say what happens after `expired_at` passes or how SARIF treats suppressed results | https://trivy.dev/latest/docs/configuration/filtering/ |
| gitleaks README | Built-in `json`, `csv`, `junit`, `sarif` reports; `--baseline-path` reports only findings not in a previous report; fingerprint ignore file | https://github.com/gitleaks/gitleaks |
| Wazuh custom SCA policies | Checks as data: file-exists, content regex (`f:path -> r:REGEX`), negation, directory scans | https://documentation.wazuh.com/current/user-manual/capabilities/sec-config-assessment/creating_custom_policies.html |
| SARIF 2.1.0 spec and community reports | `suppressions` with `kind: external` and `justification` are part of the spec; `security-severity` on the rule drives GitHub severity; several projects report GitHub code scanning ignores `suppressions` (not confirmed against GitHub docs) | https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html |

Built from these (all on `image-audit`, all read-only):

1. `--scope identity` (IDENT-001..014): the virt-sysprep checklist as an audit, so
   a golden image can be checked without running virt-sysprep. Resolves research
   item 5 above.
2. Waivers (`--waivers`) with required reason, optional expiry, stale and expired
   reporting, plus `--baseline` (gitleaks and Trivy style). Expired and stale
   entries are surfaced rather than silently ignored, which the Trivy docs leave
   unspecified.
3. `-o sarif` (SARIF 2.1.0) with stable fingerprints and suppressions.
4. `--rules FILE`: user rules as YAML data, a small Wazuh-SCA-style vocabulary
   (resolves research item 6 for simple file/line/mode rules; not an OVAL/SCAP
   engine).
5. Directory targets for `image-audit` / `image-audit-diff` (oscap-chroot style
   input), which also makes the whole rule engine testable at CLI level without
   an image.

Still not built: SCAP/OVAL content, auditing LVM or LUKS inner filesystems
without libguestfs, and CIS benchmark mapping (rule ids are not tagged with CIS
control numbers; claiming that mapping without the benchmark text in hand would
be guesswork).
