# 05 — Cutover weekend runbook

**Goal:** Hour-by-hour checklist so assurance → repair → Passport → convert → smoke is boring.

---

## T−7 days

- [ ] Wave list frozen; disks exported or snapshot IDs recorded  
- [ ] Worker image pinned; mirror/cache verified  
- [ ] Floor + signing policy agreed (`fail-below`, `--require-signature`?)  
- [ ] hyper2kvm / target cluster capacity confirmed  
- [ ] ZyAIQAAgent (or other) smoke URL ready for post-boot  
- [ ] Comms: bridge, escalation, rollback owner  

## T−1 day

- [ ] `fleet analyze` on wave; open tickets for red disks  
- [ ] Repair loop complete for all in-scope disks  
- [ ] Dry-run: `passport verify` green on copies  
- [ ] Temp passwords vaulted; domain-leave plan reviewed with IAM  

## T−0 morning (assurance)

```bash
for d in ./wave/*.qcow2; do
  guestkit passport emit "$d" --target "$TARGET" -o "${d%.qcow2}.passport.json" --bundle
  guestkit passport verify "${d%.qcow2}.passport.json" --fail-below 80
done
```

- [ ] Attach Passports to change tickets  
- [ ] Convert job blocked until all verifies exit 0  

## Convert window

- [ ] Convert only Passport-green disks  
- [ ] Do not “skip verify for one VIP” without CAB exception logged  
- [ ] First boot: confirm RDP/SSH/guest-agent per day-0 plan  

## Post-boot smoke

- [ ] App smoke (ZyAIQAAgent / manual)  
- [ ] Rotate temp local admin passwords  
- [ ] Rejoin domain if left offline  
- [ ] Leave Passport + doctor logs in ticket for audit  

## Rollback triggers

- BitLocker surprise → stop wave, decrypt path, re-assure  
- Mass Passport failure after “small” repair → halt convert, re-fleet  
- Target cluster storage full → stop; do not orphan half-converted set  

## T+1

- [ ] Blameless notes: floor misses, flake, mirror gaps  
- [ ] Update runbooks 02/03/06 if a new failure mode appeared  
