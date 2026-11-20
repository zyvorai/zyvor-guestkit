# 04 — Fleet analyze at scale

**Goal:** Score a directory of disks, find snowflakes and low-score blockers before wave planning.

---

## 1. Basic command

```bash
export GUESTKIT_FLEET_JOBS=8
guestkit fleet analyze ./images/ | tee fleet.json
# or path layout your team uses for exports
```

Clusters OS fingerprints, flags outliers and low doctor scores. Use output to order waves (easy Linux first, BitLocker/domain Windows later) — or let `fleet wave-plan` derive the order from real dependency signals (DB role, NFS storage mounts) instead of doing it by hand:

```bash
guestkit fleet wave-plan ./images/ | tee waves.json
```

See [migration-assurance.md](../features/migration-assurance.md#guestkit-fleet-wave-plan--dependency-aware-migration-ordering).

TUI:

```bash
guestctl tui ./images/some.qcow2 --fleet ./images/
guestctl tui a.qcow2 --compare b.qcow2
```

---

## 2. Pipeline sketch

1. Export / sync wave N disks to scratch.  
2. `fleet analyze` → artifact `fleet-wave-N.json`.  
3. Ticket per blocker (score &lt; floor, BitLocker, missing VirtIO plan).  
4. Per-disk repair loop (runbook 02) until Passport green.  
5. Only then schedule convert for that wave.

---

## 3. Capacity

| Disks | Guidance |
|-------|----------|
| &lt; 20 | Single worker, `GUESTKIT_FLEET_JOBS=4` |
| 20–100 | Dedicated scratch SSD, jobs=8, stagger I/O |
| 100+ | Shard directories by wave; multiple workers; object-store pull |

Don’t run fleet analyze on the same spindles as live production storage.

---

## 4. Policy-as-code

Combine fleet output with your org policy (required packages, forbidden domain-join at cutover, min score). Fail the wave gate if any disk remains below floor after repair SLA.

Related blog: [Fleet inspect-batch](https://zyvor.dev/blog/guestkit-fleet-inspect-batch).
