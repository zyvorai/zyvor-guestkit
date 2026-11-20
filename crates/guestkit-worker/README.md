# guestkit-worker

**Distributed Worker for VM Operations**

This crate provides the worker implementation for executing VM operations jobs defined by the guestkit-job-spec protocol.

## Features

- 🔌 **Pluggable Transport** - File-based (REST/Queue coming soon)
- 🎯 **Handler Registry** - Plugin system for operations
- 📊 **Progress Tracking** - Real-time job progress
- ♻️ **Idempotent Execution** - Safe retries with idempotency keys
- ⏱️ **Timeout Support** - Configurable job timeouts
- 📝 **Result Persistence** - Structured job results
- 🔄 **State Machine** - Proper state transitions
- 🛡️ **Graceful Shutdown** - Clean worker termination

## Quick Start

### Running the Worker

```bash
# Start worker with default settings
cargo run --bin guestkit-worker

# With custom configuration
cargo run --bin guestkit-worker -- \
    --worker-id worker-01 \
    --pool production \
    --jobs-dir ./jobs \
    --results-dir ./results \
    --max-concurrent 8
```

### Submitting a Job

```bash
# Create a test job
cat > jobs/test-job-123.json <<EOF
{
  "version": "1.0",
  "job_id": "test-job-123",
  "created_at": "2026-01-30T10:00:00Z",
  "kind": "VMOperation",
  "operation": "system.echo",
  "payload": {
    "type": "system.echo.v1",
    "data": {
      "message": "Hello from guestkit worker!"
    }
  }
}
EOF

# Worker will pick it up automatically
# Result will be in: results/test-job-123-result.json
```

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    Worker Daemon                    │
│  ┌───────────────────────────────────────────────┐  │
│  │  Transport Layer (File/REST/Queue)            │  │
│  │  - Fetch jobs                                 │  │
│  │  - Acknowledge completion                     │  │
│  └───────────────────────────────────────────────┘  │
│                        ↓                             │
│  ┌───────────────────────────────────────────────┐  │
│  │  Job Executor                                 │  │
│  │  - Validate job                               │  │
│  │  - Check idempotency                          │  │
│  │  - Execute with timeout                       │  │
│  │  - State machine transitions                  │  │
│  └───────────────────────────────────────────────┘  │
│                        ↓                             │
│  ┌───────────────────────────────────────────────┐  │
│  │  Handler Registry                             │  │
│  │  - Route to correct handler                   │  │
│  │  - Execute operation                          │  │
│  │  - Track progress                             │  │
│  └───────────────────────────────────────────────┘  │
│                        ↓                             │
│  ┌───────────────────────────────────────────────┐  │
│  │  Result Writer                                │  │
│  │  - Persist results                            │  │
│  │  - Cache idempotency keys                     │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

## Components

### Transport Layer

Pluggable job sources:

- **FileTransport** - Watch directory for job files (v1)
- **RestTransport** - HTTP API polling (future)
- **QueueTransport** - Kafka/Redis pub/sub (future)

### Handler Registry

Plugin system for operations:

```rust
use guestkit_worker::{HandlerRegistry, OperationHandler};
use async_trait::async_trait;

struct MyHandler;

#[async_trait]
impl OperationHandler for MyHandler {
    fn name(&self) -> &str { "my-handler" }
    fn operations(&self) -> Vec<String> {
        vec!["custom.operation".to_string()]
    }
    async fn execute(
        &self,
        context: HandlerContext,
        payload: Payload,
    ) -> WorkerResult<HandlerResult> {
        // Your operation logic
        Ok(HandlerResult::new())
    }
}

// Register handler
let mut registry = HandlerRegistry::new();
registry.register(Arc::new(MyHandler));
```

### Progress Tracking

Real-time progress reporting:

```rust
// In your handler
context.report_progress(
    "processing",
    Some(50),
    "Processing VM disk"
).await?;
```

### State Machine

Valid state transitions:

```
Pending → Queued → Assigned → Running → Completed
                                      → Failed
                                      → Timeout
                                      → Cancelled
```

## Built-in Handlers

### Echo Handler

Test handler that echoes back the payload:

```bash
# Create test job
cat > jobs/echo-test.json <<EOF
{
  "version": "1.0",
  "job_id": "echo-test",
  "created_at": "2026-01-30T10:00:00Z",
  "kind": "VMOperation",
  "operation": "system.echo",
  "payload": {
    "type": "system.echo.v1",
    "data": {"message": "test"}
  }
}
EOF
```

## Configuration

### Worker Config

```rust
WorkerConfig {
    worker_id: "worker-01".to_string(),
    worker_pool: Some("production".to_string()),
    work_dir: PathBuf::from("/tmp/worker"),
    result_dir: PathBuf::from("./results"),
    max_concurrent_jobs: 4,
    shutdown_timeout_secs: 30,
}
```

### Capabilities

```rust
Capabilities::new()
    .with_operation("guestkit.inspect")
    .with_operation("guestkit.profile")
    .with_feature("lvm")
    .with_feature("nbd")
    .with_disk_format("qcow2")
```

## Testing

```bash
# Run unit tests
cargo test

# Run with logging
RUST_LOG=debug cargo test -- --nocapture

# Run specific test
cargo test test_executor
```

## Integration with guestkit

To add guestkit operations:

1. Create handler in `src/handlers/inspect.rs`:

```rust
pub struct InspectHandler {
    // guestkit integration
}

#[async_trait]
impl OperationHandler for InspectHandler {
    fn operations(&self) -> Vec<String> {
        vec!["guestkit.inspect".to_string()]
    }

    async fn execute(
        &self,
        context: HandlerContext,
        payload: Payload,
    ) -> WorkerResult<HandlerResult> {
        // Call guestkit library
        // Report progress
        // Return results
    }
}
```

2. Register in worker binary:

```rust
registry.register(Arc::new(InspectHandler::new()));
```

## Monitoring

Worker emits structured logs:

```
[INFO] Starting worker worker-01
[INFO] Watching for jobs in: ./jobs
[INFO] Received job: test-job-123
[INFO] [test-job-123] validation - Validating job (0%)
[INFO] [test-job-123] execution - Running operation (50%)
[INFO] [test-job-123] completion - Job completed (100%)
[INFO] Job test-job-123 completed successfully
```

## Roadmap

- [ ] REST transport (HTTP polling)
- [ ] Queue transport (Kafka/Redis)
- [ ] Metrics export (Prometheus)
- [ ] Distributed tracing (OpenTelemetry)
- [ ] Job scheduler integration
- [ ] Resource limits (CPU/memory)
- [ ] Health check endpoint
- [ ] Worker registration service

## License

Zyvor Production License v1.0
