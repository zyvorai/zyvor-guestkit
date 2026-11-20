# 10 — Rescue dry-run and SBOM diff in CI

## Passport gate + rescue plan + SBOM

```yaml
- uses: zyvorai/guestkit@main
  with:
    disk: images/web.qcow2
    target: kvm
    fail-below: '80'
    rescue: enable-ssh
    rescue-user: ubuntu
    sbom: 'true'
    handoff: 'true'
```

Rescue uses `guestkit rescue … --export-plan`. The disk is not mutated.
Apply later with `guestkit plan apply rescue.yaml --vm images/web.qcow2 --yes`.

## Rescue-only composite

```yaml
- uses: zyvorai/guestkit/.github/actions/rescue-dry-run@main
  with:
    disk: images/web.qcow2
    operation: fix-fstab
```

## SBOM drift gate

```bash
guestkit inventory before.qcow2 --format spdx -o before.spdx.json
guestkit inventory after.qcow2  --format spdx -o after.spdx.json
guestkit sbom-diff before.spdx.json after.spdx.json --fail-on-drift

guestkit forensic-diff before.qcow2 after.qcow2 \
  --sbom-old before.spdx.json --sbom-new after.spdx.json -o json
```

`--fail-on-drift` exits 1 on added, removed, or version-changed packages.
