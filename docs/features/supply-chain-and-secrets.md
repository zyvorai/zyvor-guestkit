# Supply chain, vulnerability reporting and secrets scanning

Everything on this page works offline. Feeds are plain files you supply (or
import from a bundle); nothing is fetched at report time.

## `guestkit cve`

```bash
# Inventory JSON from `guestkit inventory IMAGE -f json --include-cves`, or an image directly
guestkit cve inventory.json
guestkit cve inventory.json -f openvex -o report.openvex.json
guestkit cve inventory.json -f csaf    -o report.csaf.json
```

| Flag | Effect |
|---|---|
| `-f text\|json\|openvex\|csaf` | Output format. OpenVEX v0.2.0 and CSAF 2.0 (`csaf_vex`). |
| `--vex FILE` | OpenVEX input. Findings with a matching `not_affected` or `fixed` statement are suppressed (reported as such in OpenVEX/CSAF/JSON, ignored by the gate). Later statements override earlier ones; `affected` / `under_investigation` keep a finding active. Products match by purl (`pkg:rpm/name@version`, version optional) or package name; no products means all. Aliases are honoured. |
| `--epss FILE` | EPSS scores from a local file: FIRST CSV (`cve,epss,percentile`), FIRST API JSON, or a JSON map. |
| `--kev FILE` | CISA KEV catalog JSON, a JSON array of CVE ids, or one id per line. |
| `--fail-on SEVERITY` | Exit code 2 when an active finding is at or above `critical`, `high`, `medium` or `low`. Findings with unknown severity use the CVSS score when present. |
| `--fail-on-kev` | Exit code 2 when an active finding is in KEV. |
| `--offline` / `GUESTKIT_OFFLINE=1` | Never call the OSV API; use the cache and imported bundles only (and do not write empty results to the cache). |

If `--epss` / `--kev` are not given, feeds imported with `--db-import` are read
from `<cache-root>/feeds/` (`epss.json`, `kev.json`).

### Air-gapped vulnerability DB bundle

```bash
# online host: after normal lookups have populated the cache, and with feeds in <cache>/feeds
guestkit cve --db-export vulndb.json
# transfer vulndb.json, then on the air-gapped host
guestkit cve --db-import vulndb.json
GUESTKIT_OFFLINE=1 guestkit cve inventory.json --fail-on high
```

The bundle (`guestkit-vulndb/1`) holds the OSV lookup cache (keyed like the
cache files), EPSS scores and the KEV list, with a SHA-256 over the payload.
Import refuses a bundle whose digest does not match. The digest detects
corruption and casual tampering; it is not a signature (sign the file out of
band if you need provenance).

## `guestkit attest`

An in-toto Statement v1 whose subject is the image file's SHA-256 and whose
predicate is a CycloneDX or SPDX document, wrapped in a DSSE envelope and
signed with Ed25519, using the same key files as `passport keygen`.

```bash
guestkit passport keygen --seed attest.key --public attest.pub   # existing command
guestkit attest create disk.qcow2 --sign-key attest.key -o disk.att.json            # generates CycloneDX
guestkit attest create disk.qcow2 --sbom my-sbom.spdx.json --sign-key attest.key -o disk.att.json
guestkit attest verify disk.att.json --image disk.qcow2 --public-key "$(cat attest.pub)"
guestkit attest verify disk.att.json --trust-keys trusted-keys.txt
```

`verify` checks the DSSE signature against an explicitly trusted key
(`--public-key` or `--trust-keys`; it will not trust the key named inside the
envelope), the payload type, the in-toto structure and, with `--image`, that
the file's current SHA-256 equals a subject digest. Signing and verification need
a build with `--features agent` (same as passport signing).

Not included: Sigstore / keyless signing, Rekor transparency log, cosign key
formats. The envelope is plain DSSE; cosign compatibility (ECDSA P-256 keys,
`cosign verify-blob-attestation`) is a follow-up.

## `guestkit secrets` v2

```bash
guestkit secrets disk.qcow2                                   # built-in rules + entropy, redacted
guestkit secrets disk.qcow2 --format sarif -o secrets.sarif
guestkit secrets disk.qcow2 --rules corp-rules.toml --baseline secrets.baseline.json --fail-on-findings
guestkit secrets disk.qcow2 --write-baseline secrets.baseline.json    # accept the current findings
```

* **Default paths**: `/etc /home /root /var/www /opt /srv /var/lib/cloud /usr/local/etc`
  (override with `--scan-paths`). Rules cover AWS/Azure/GCP-style credentials,
  `~/.kube/config`, `~/.docker/config.json`, `.netrc`, `.pgpass`, `.npmrc`,
  `.git-credentials`, GitHub/GitLab/Slack/Stripe tokens, JWTs, PEM/OpenSSH
  private keys, and passwords/secrets exported or passed on the command line in
  shell history. `/node_modules/`, `/usr/share/`, CA certificate directories
  are excluded unless `--exclude` is given.
* **Entropy**: any assigned value (`key = value`, `key: "value"`) of 20+
  characters whose Shannon entropy exceeds `--entropy-threshold` (default 4.5
  bits/char) is reported as `generic-high-entropy`, unless a specific rule
  already reported it. `--no-entropy` disables it. Expect some false positives
  on base64 blobs; use allowlists or a baseline.
* **Rule files** (`--rules`, gitleaks-style TOML):

```toml
[extend]
useDefault = true          # false: only the rules below

[[rules]]
id = "acme-token"
description = "ACME API token"
regex = '''acme_([a-z0-9]{16})'''
secretGroup = 1            # capture group holding the secret (0 = whole match)
keywords = ["acme_"]       # cheap line prefilter
entropy = 3.0              # minimum entropy of the secret
path = '''\.env$'''         # only files whose guest path matches
severity = "critical"
[rules.allowlist]
stopwords = ["test"]

[allowlist]                # global
paths = ['''/vendor/''']
regexes = ['''^dummy''']
stopwords = ["sample"]
```

  `--allow-path` / `--allow-regex` add global allowlist entries; a line
  containing `guestkit:allow` (or `gitleaks:allow`) is skipped. Built-in
  stopwords skip obvious placeholders (`changeme`, `${...}`, `redacted`, ...).
  Lookaround is not available (Rust `regex` crate).
* **Baseline**: each finding has a fingerprint (SHA-256 of rule, file and
  secret, truncated; independent of line number and not reversible to the
  secret). `--write-baseline` records current findings (extending an existing
  file); `--baseline` suppresses them on later runs.
* **Output**: `--format text|json|sarif`, `-o FILE`. Secrets are always
  redacted (`ab***[REDACTED 24 chars]`); SARIF and JSON never contain the
  secret unless `--show-content` is passed (JSON only; SARIF always redacted).
  `--fail-on-findings` exits 2 when findings remain.

## Library entry points (for bindings)

* `cli::inventory::vuln_report`: `collect_findings`, `parse_openvex`, `apply_vex`, `parse_epss`, `parse_kev`, `Enrichment::apply`, `to_openvex`, `to_csaf`, `count_at_or_above`, `export_bundle`, `import_bundle`
* `assurance::attest`: `sha256_file`, `build_statement`, `sign_statement`, `verify_envelope`
* `cli::secrets_v2`: `Scanner::from_args`, `Scanner::scan_text`, `to_sarif`, `load_baseline`, `write_baseline`, `apply_baseline`
