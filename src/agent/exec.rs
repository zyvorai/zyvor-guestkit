// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Synchronous guest command execution for VMRogue (replaces K8s guest-exec subresource).

use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::process::{Command, Stdio};

/// Exit code of a finished process. A process killed by a signal has no exit
/// code; report the shell convention 128+signal instead of a generic 1.
fn exit_code(status: &std::process::ExitStatus) -> i64 {
    if let Some(c) = status.code() {
        return c as i64;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig as i64;
        }
    }
    1
}

/// Run a command synchronously and return exit code + stdout/stderr.
pub fn exec_sync(args: &Value) -> Result<Value, String> {
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing path".to_string())?;

    let mut cmd = Command::new(path);
    if let Some(arr) = args.get("arg").and_then(|v| v.as_array()) {
        for a in arr {
            if let Some(s) = a.as_str() {
                cmd.arg(s);
            }
        }
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = cmd.output().map_err(|e| format!("exec: {e}"))?;

    Ok(json!({
        "exited": true,
        "exitcode": exit_code(&output.status),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
    }))
}

/// QGA-compatible synchronous exec returning base64 out-data / err-data.
pub fn exec_sync_qga(args: &Value) -> Result<Value, String> {
    let inner = exec_sync(args)?;
    let mut ret = json!({
        "exited": true,
        "exitcode": inner.get("exitcode").and_then(|v| v.as_i64()).unwrap_or(1),
    });
    if let Some(stdout) = inner.get("stdout").and_then(|v| v.as_str()) {
        if !stdout.is_empty() {
            ret["out-data"] = Value::String(STANDARD.encode(stdout.as_bytes()));
        }
    }
    if let Some(stderr) = inner.get("stderr").and_then(|v| v.as_str()) {
        if !stderr.is_empty() {
            ret["err-data"] = Value::String(STANDARD.encode(stderr.as_bytes()));
        }
    }
    Ok(ret)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn signal_killed_process_reports_128_plus_signal() {
        let r = exec_sync(&json!({"path": "/bin/sh", "arg": ["-c", "kill -9 $$"]})).unwrap();
        assert_eq!(r["exitcode"], 137);
    }

    #[test]
    fn normal_exit_code_is_preserved() {
        let r = exec_sync(&json!({"path": "/bin/sh", "arg": ["-c", "exit 3"]})).unwrap();
        assert_eq!(r["exitcode"], 3);
    }
}
