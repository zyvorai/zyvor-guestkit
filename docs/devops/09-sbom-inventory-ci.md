# 09 — SBOM / inventory in CI

**Goal:** Emit SPDX/CycloneDX (and optional CVE/license views) as pipeline artifacts beside Passport.

```bash
guestkit inventory "$DISK" --format spdx --include-licenses --include-cves \
  -o "sbom-${CI_JOB_ID}.spdx.json"
guestkit inspect "$DISK" --export markdown > "inventory-${CI_JOB_ID}.md"
```

Gate with your org SBOM scanner or policy engine. Regenerate on every golden bake; retain with change tickets.

See [SBOM blog](https://zyvor.dev/blog/guestkit-sbom-inventory).
