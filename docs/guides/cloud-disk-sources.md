# Cloud disk sources (S3 / GCS / Azure)

GuestKit can pull VM disks from object storage before offline inspect/doctor/passport.

Rebuild with the matching features (or `--features cloud` for all three):

```bash
cargo build --release --features cloud-s3
cargo build --release --features cloud-azure
cargo build --release --features cloud-gcs
cargo build --release --features cloud
```

## URI forms

| Backend | URI | CLI tool |
|---------|-----|----------|
| S3 / MinIO | `s3://bucket/path/vm.qcow2` | `aws` |
| GCS | `gs://bucket/path/vm.qcow2` | `gsutil` or `gcloud storage` |
| Azure | `https://acct.blob.core.windows.net/container/blob` or `azure://acct/container/blob` | `az` |

```bash
guestkit doctor s3://my-bucket/fleet/win10.qcow2 --target kvm
guestkit inspect gs://my-bucket/images/rhel9.qcow2
guestkit passport emit azure://acct/disks/vm.vhd --target kvm -o passport.json
```

## Local download cache

Downloads land under `~/.cache/guestkit/cloud/` (keyed by URI hash) so repeat pulls skip the network.

| Env | Meaning |
|-----|---------|
| `GUESTKIT_CLOUD_CACHE` | `0`/`false` disables cache (temp file, deleted after use) |
| `GUESTKIT_CLOUD_CACHE_DIR` | Override cache directory |

## S3 / MinIO endpoints

| Env | Meaning |
|-----|---------|
| `GUESTKIT_S3_ENDPOINT` | Passed as `aws s3 cp --endpoint-url` (preferred) |
| `AWS_ENDPOINT_URL` | Fallback endpoint URL |

```bash
export GUESTKIT_S3_ENDPOINT=http://127.0.0.1:9000
export AWS_ACCESS_KEY_ID=zyvor
export AWS_SECRET_ACCESS_KEY=zyvor-secret
guestkit doctor s3://guestkit/fixtures/tiny.qcow2
```

For path-style MinIO addressing, configure the AWS CLI (`aws configure set default.s3.addressing_style path`).

## CI recipe (optional live pull)

The feature matrix script exercises live URIs when credentials + tools exist:

```bash
export GK_TEST_S3_URI=s3://bucket/fixture.qcow2
export GK_TEST_AZURE_URI=https://acct.blob.core.windows.net/c/fixture.qcow2
export GK_TEST_GCS_URI=gs://bucket/fixture.qcow2
./scripts/test-feature-matrix.sh
```

Unit coverage (no cloud credentials):

```bash
cargo test -p guestkit --lib storage::cloud_cache
cargo test -p guestkit --features cloud-azure --lib storage::azure::tests
```

See also [scripts/ci-cloud-disk-sources.sh](../../scripts/ci-cloud-disk-sources.sh).
