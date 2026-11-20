# 06 — Failure triage (GuestKit)

**Goal:** Doctor/Passport red — classify in minutes and route.

---

## 1. Grab evidence

```bash
guestkit doctor disk.qcow2 --target "$TARGET" --explain | tee doctor.txt
# passport verify stderr + passport.json
jq . passport.json
```

Also: migrate-plan YAML, rescue command used, package mode env, build features (`registry-write`?).

---

## 2. Decision tree

```text
BitLocker / encrypted volume
  → hard offline block — unlock/decrypt export; re-run doctor
Score low: GRUB / BCD / EFI
  → rescue fix-grub --force; re-doctor
Score low: missing guest tools / VirtIO
  → packages mirror + GUESTKIT_VIRTIO_WIN; plan apply
Windows password / domain trust
  → reset-password (AES SAM or RunOnce); domain-leave plan
PackageInstall failed
  → cache/mirror/fetch mode; package name present?
passport verify signature fail
  → trust-keys / clock / expired emit — DevOps signing
Works on laptop, fails in CI
  → privileges (nbd), disk path, image pin mismatch
```

---

## 3. Classify & page

| Class | Owner |
|-------|--------|
| Infra worker / NBD / image pull | DevOps |
| OS bootloader / fstab | Linux eng |
| Windows SAM / domain | Windows eng + IAM |
| Package mirror empty | DevOps + packaging |
| Convert after green Passport fails | hyper2kvm / platform (not GuestKit score) |

---

## 4. Re-verify protocol

1. Repair on a **copy**.  
2. Re-doctor → re-emit Passport (do not reuse stale signed Passport after mutate).  
3. `passport verify` again.  
4. Only then unlock convert.  

Stale Passport after disk change = audit fail even if score “looks” fine.

---

## 5. Known limits (don’t fight the tool)

- `check-grub` diagnose-only in some modes — use rescue `fix-grub` to repair  
- DC computer-object delete needs live AD  
- AES SAM needs registry-write build; else RunOnce  
- Packages need real cache/dnf|apt/mirror content  
