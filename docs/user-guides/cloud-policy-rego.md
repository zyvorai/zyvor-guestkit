# Cloud profiles and Rego policy

## Cloud cutover profiles

```bash
guestkit cloud-profile aws
guestkit cloud-profile azure --export azure-policy.yaml
guestkit cloud-profile gcp --image disk.qcow2 --strict
guestkit policy check disk.qcow2 -b aws --strict
```

Profiles (`aws`, `azure`, `gcp`, `openstack`) are ordinary GuestKit Policy
packs: cloud-init present, telnet absent (AWS), boot score ≥ 80. They do
not call cloud APIs.

## Rego deny rules

```bash
guestkit passport emit disk.qcow2 --target kvm -o passport.json
guestkit policy rego --rego policies/cutover.rego --input passport.json --fail
```

GuestKit evaluates a small subset in-process:

```
deny[msg] {
  input.PATH OP VALUE
  msg := "reason"
}
```

`PATH` is a dotted JSON path (`scores.boot`, `hard_blocked`). Ops:
`== != < <= > >=`.

If `opa` is on `PATH` (or `$OPA_BIN`), the same file is also run through
`opa eval data.guestkit.deny` and results are merged.

## CI

```yaml
- run: guestkit policy check "$DISK" -b aws --strict
- run: guestkit policy rego --rego policies/cutover.rego --input passport.json --fail
```
