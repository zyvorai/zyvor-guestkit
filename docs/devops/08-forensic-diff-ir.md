# 08 — Forensic diff & offline IR

**Goal:** Investigate a suspicious or drifted disk **without booting** it; produce evidence for the incident ticket.

```bash
guestkit forensic-diff golden.qcow2 suspect.qcow2 -o json | tee drift.json
guestkit secrets suspect.qcow2
guestkit malware suspect.qcow2
guestkit timeline suspect.qcow2
```

1. Quarantine object-store copy → IR scratch worker.  
2. Diff vs golden / last-good.  
3. Attach JSON; decide rebuild vs repair+Passport.  
4. Never “just power on to see.”

See [forensic-diff blog](https://zyvor.dev/blog/guestkit-forensic-diff-ir).

**Scheduled, fleet-wide version:** this runbook is for a one-off, reactive diff against a specific suspicious disk. For ongoing drift monitoring across a whole fleet on a schedule, see [`guestkit fleet watch`](../features/migration-assurance.md#guestkit-fleet-watch--scheduled-drift-monitoring) instead.
