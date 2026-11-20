# 07 — Cloud disk sources (S3 / GCS / Azure)

**Goal:** Doctor / Passport disks that live in object storage without manual `aws s3 cp` every time.

---

## 1. Build features

```bash
cargo build --release --features cloud-s3
cargo build --release --features cloud-gcs
cargo build --release --features cloud-azure
# or all:
cargo build --release --features cloud
```

Worker images used in CI must include the features you need.

---

## 2. URI forms

| Backend | Example |
|---------|---------|
| S3 / MinIO | `s3://bucket/path/vm.qcow2` |
| GCS | `gs://bucket/path/vm.qcow2` |
| Azure | `https://acct.blob.core.windows.net/c/vm.qcow2` or `azure://acct/container/blob` |

```bash
guestkit doctor s3://my-bucket/fleet/win10.qcow2 --target kvm
guestkit passport emit gs://my-bucket/images/rhel9.qcow2 --target kvm -o passport.json
```

Host needs the matching CLI (`aws` / `gsutil` / `az`) and credentials (instance role, `AWS_*`, etc.).

---

## 3. Cache

| Env | Meaning |
|-----|---------|
| `GUESTKIT_CLOUD_CACHE` | `0` disables (temp file) |
| `GUESTKIT_CLOUD_CACHE_DIR` | Override `~/.cache/guestkit/cloud/` |
| `GUESTKIT_S3_ENDPOINT` / `AWS_ENDPOINT_URL` | MinIO / custom S3 |

Size scratch disks for peak concurrent pulls. Purge cache between waves if space is tight.

---

## 4. CI sketch

```bash
export AWS_REGION=us-east-1
# credentials via OIDC / instance profile — not long-lived keys in YAML
guestkit passport emit "s3://${BUCKET}/${KEY}" --target kvm -o passport.json --bundle
guestkit passport verify passport.json --fail-below 80
```

Prefer pulling once to local scratch for repair+re-emit, then upload Passport + repaired disk as separate artifacts.

Full guide: [cloud-disk-sources.md](../guides/cloud-disk-sources.md).
