# Worker Job Examples

Example job files for testing the guestkit worker.

## Quick Test

```bash
# Start the worker
cd crates/guestkit-worker
cargo run --bin guestkit-worker

# In another terminal, submit a job
mkdir -p jobs
cp ../../examples/worker-jobs/guestkit-inspect-basic.json jobs/

# Worker will:
# 1. Pick up the job
# 2. Execute it
# 3. Move to jobs/done/
# 4. Write result to results/
```

## Example Jobs

### Echo Test

**File:** `echo-test.json`
**Purpose:** Test basic worker functionality
**Operation:** `system.echo`

```bash
cp examples/worker-jobs/echo-test.json jobs/
```

### VM Inspection (Basic)

**File:** `guestkit-inspect-basic.json`
**Purpose:** Basic VM disk inspection
**Operation:** `guestkit.inspect`

```bash
cp examples/worker-jobs/guestkit-inspect-basic.json jobs/
```

Features:
- Read-only access
- Package enumeration
- Service discovery
- Network configuration
- Security settings

### VM Inspection (Full)

**File:** `guestkit-inspect-full.json`
**Purpose:** Comprehensive VM inspection with all options
**Operation:** `guestkit.inspect`

```bash
cp examples/worker-jobs/guestkit-inspect-full.json jobs/
```

Features:
- Deep scan enabled
- All modules included
- Checksum verification
- Idempotency key
- Full observability

### Security Profile

**File:** `guestkit-profile-security.json`
**Purpose:** Security and compliance scanning
**Operation:** `guestkit.profile`

```bash
cp examples/worker-jobs/guestkit-profile-security.json jobs/
```

Profiles:
- Security vulnerabilities
- Compliance checks (PCI-DSS)
- Hardening recommendations

## Job Structure

All jobs follow the standard protocol:

```json
{
  "version": "1.0",
  "job_id": "unique-job-id",
  "created_at": "2026-01-30T16:00:00Z",
  "kind": "VMOperation",
  "operation": "operation.name",
  "payload": {
    "type": "operation.name.v1",
    "data": { /* operation-specific */ }
  }
}
```

## Customizing Jobs

### Change Image Path

```json
{
  "payload": {
    "data": {
      "image": {
        "path": "/your/path/to/vm.qcow2"
      }
    }
  }
}
```

### Add Idempotency

```json
{
  "execution": {
    "idempotency_key": "your-unique-key"
  }
}
```

### Set Priority

```json
{
  "execution": {
    "priority": 9  // 1-10, higher = more urgent
  }
}
```

### Add Metadata

```json
{
  "metadata": {
    "name": "my-job",
    "labels": {
      "environment": "prod",
      "team": "platform"
    }
  }
}
```

## Expected Results

After job execution, check `results/` directory:

```bash
# View result
cat results/inspect-basic-001-result.json

# Expected structure
{
  "job_id": "inspect-basic-001",
  "status": "completed",
  "worker_id": "worker-xxx",
  "execution_summary": {
    "started_at": "...",
    "duration_seconds": 5
  },
  "outputs": {
    "primary": "/tmp/inspect-result.json"
  }
}
```

## Troubleshooting

### Job Stuck in jobs/

Worker may not be running or job may be invalid.

Check worker logs:
```bash
RUST_LOG=debug cargo run --bin guestkit-worker
```

### Job in jobs/failed/

Check failure reason:
```bash
cat jobs/failed/job-id.reason.txt
```

### No Result File

Check if job completed:
```bash
ls -la jobs/done/
ls -la results/
```

## Advanced Usage

### Batch Submission

```bash
# Submit multiple jobs
for i in {1..10}; do
  sed "s/JOB_ID/job-$i/" template.json > jobs/job-$i.json
done
```

### Concurrent Execution

Worker processes jobs concurrently (default: 4 max).

Adjust in worker config:
```bash
cargo run --bin guestkit-worker -- --max-concurrent 8
```

## See Also

- [Worker README](../../crates/guestkit-worker/README.md)
- [Documentation](../../docs/INDEX.md)
- [Job Examples](../../examples/jobs/)
