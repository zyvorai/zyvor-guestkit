# guestkit-job-spec

**VM Operations Job Protocol Specification and Types**

This crate provides the type definitions, validation, and utilities for the VM Operations Job Protocol v1. It supports creating, validating, and serializing job specifications for distributed VM operations.

## Features

- 🏗️ **Type-safe job specifications** - Strongly-typed Rust structs with serde support
- ✅ **Validation** - Comprehensive job validation before execution
- 🔨 **Fluent builder** - Easy-to-use builder pattern for creating jobs
- 📦 **Transport agnostic** - Works with files, REST APIs, message queues
- 🔄 **Forward compatible** - Unknown fields are preserved
- 📝 **Well-documented** - Complete API documentation

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
guestkit-job-spec = "0.1"
```

### Creating a Job

```rust
use guestkit_job_spec::builder::JobBuilder;

let job = JobBuilder::new()
    .job_id("job-12345")
    .operation("guestkit.inspect")
    .payload("guestkit.inspect.v1", serde_json::json!({
        "image": {
            "path": "/vms/test.qcow2",
            "format": "qcow2"
        }
    }))
    .name("inspect-test-vm")
    .label("env", "test")
    .priority(5)
    .timeout_seconds(3600)
    .require_capability("guestkit.inspect")
    .build()?;

// Serialize to JSON
let json = serde_json::to_string_pretty(&job)?;
```

### Using the Helper

```rust
use guestkit_job_spec::builder::inspect_job;

let job = inspect_job("/vms/production.qcow2")
    .name("weekly-scan")
    .priority(7)
    .worker_pool("pool-prod")
    .build()?;
```

### Validation

```rust
use guestkit_job_spec::{JobValidator, JobDocument};

let job: JobDocument = serde_json::from_str(&json_string)?;

// Validate the job
JobValidator::validate(&job)?;

// Check capability matching
let required = vec!["guestkit.inspect".to_string()];
let available = vec!["guestkit.inspect".to_string(), "guestkit.fix".to_string()];
JobValidator::check_capabilities(&required, &available)?;
```

### Deserialization

```rust
use guestkit_job_spec::JobDocument;
use std::fs;

// From file
let json = fs::read_to_string("job.json")?;
let job: JobDocument = serde_json::from_str(&json)?;

// Validate after deserialization
JobValidator::validate(&job)?;
```

## Job Structure

A job document consists of:

- **Envelope** - Stable control plane (version, job_id, operation, etc.)
- **Metadata** - Labels, annotations, namespaces
- **Execution** - Retry policy, timeouts, priorities
- **Constraints** - Required capabilities and features
- **Routing** - Worker pool selection and affinity
- **Payload** - Operation-specific data (guestkit.inspect.v1, etc.)
- **Observability** - Trace IDs, correlation IDs
- **Audit** - Submitter, authorization

## Operations

Supported operation namespaces:

### Guestkit Operations

- `guestkit.inspect` - VM disk inspection
- `guestkit.profile` - Security/compliance profiling
- `guestkit.fix` - Offline repair operations
- `guestkit.convert` - Disk format conversion
- `guestkit.compare` - VM comparison

### Future Operations

- `hyper2kvm.convert` - VM migration and conversion
- `hyper2kvm.validate` - Migration validation
- `system.health-check` - Worker health checks

## Examples

See the [examples/jobs](../../examples/jobs/) directory for complete examples:

- `inspect-minimal.json` - Minimal inspection job
- `inspect-full.json` - Full-featured inspection with all options
- `profile-security.json` - Security profiling job
- `fix-offline.json` - Offline repair job

## Documentation

- [Documentation](../../docs/INDEX.md)
- [API Documentation](https://docs.rs/guestkit-job-spec) - Rust API docs

## License

Zyvor Production Licence v2.0
