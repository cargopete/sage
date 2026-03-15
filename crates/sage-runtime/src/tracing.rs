//! Tracing support for Sage programs.
//!
//! Emits newline-delimited JSON (NDJSON) trace events to stderr or a file.

use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Global flag indicating whether tracing is enabled.
static TRACING_ENABLED: AtomicBool = AtomicBool::new(false);

/// Global trace output (file or stderr).
static TRACE_OUTPUT: Mutex<Option<TraceOutput>> = Mutex::new(None);

enum TraceOutput {
    Stderr,
    File(std::fs::File),
}

/// Initialize tracing from environment variables.
///
/// Checks SAGE_TRACE and SAGE_TRACE_FILE environment variables.
pub fn init() {
    if std::env::var("SAGE_TRACE").is_ok() || std::env::var("SAGE_TRACE_FILE").is_ok() {
        TRACING_ENABLED.store(true, Ordering::SeqCst);

        let output = if let Ok(path) = std::env::var("SAGE_TRACE_FILE") {
            match OpenOptions::new().create(true).append(true).open(&path) {
                Ok(file) => TraceOutput::File(file),
                Err(e) => {
                    eprintln!("Warning: Could not open trace file {}: {}", path, e);
                    TraceOutput::Stderr
                }
            }
        } else {
            TraceOutput::Stderr
        };

        *TRACE_OUTPUT.lock().unwrap() = Some(output);
    }
}

/// Check if tracing is enabled.
#[inline]
pub fn is_enabled() -> bool {
    TRACING_ENABLED.load(Ordering::Relaxed)
}

/// Get current timestamp in milliseconds.
fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A trace event.
#[derive(Serialize)]
struct TraceEvent<'a> {
    /// Timestamp in milliseconds since Unix epoch.
    t: u64,
    /// Event kind.
    kind: &'a str,
    /// Additional fields (flattened).
    #[serde(flatten)]
    data: serde_json::Value,
}

/// Emit a trace event.
fn emit_event(kind: &str, data: serde_json::Value) {
    if !is_enabled() {
        return;
    }

    let event = TraceEvent {
        t: timestamp_ms(),
        kind,
        data,
    };

    if let Ok(json) = serde_json::to_string(&event) {
        let mut guard = TRACE_OUTPUT.lock().unwrap();
        if let Some(ref mut output) = *guard {
            let line = format!("{}\n", json);
            match output {
                TraceOutput::Stderr => {
                    let _ = std::io::stderr().write_all(line.as_bytes());
                }
                TraceOutput::File(f) => {
                    let _ = f.write_all(line.as_bytes());
                }
            }
        }
    }
}

/// Trace an agent spawn event.
pub fn agent_spawn(agent: &str, id: &str) {
    emit_event(
        "agent.spawn",
        serde_json::json!({
            "agent": agent,
            "id": id,
        }),
    );
}

/// Trace an agent emit event.
pub fn agent_emit(agent: &str, id: &str, value_type: &str) {
    emit_event(
        "agent.emit",
        serde_json::json!({
            "agent": agent,
            "id": id,
            "value_type": value_type,
        }),
    );
}

/// Trace an agent stop event.
pub fn agent_stop(agent: &str, id: &str, duration_ms: u64) {
    emit_event(
        "agent.stop",
        serde_json::json!({
            "agent": agent,
            "id": id,
            "duration_ms": duration_ms,
        }),
    );
}

/// Trace an agent error event.
pub fn agent_error(agent: &str, id: &str, error_kind: &str, message: &str) {
    emit_event(
        "agent.error",
        serde_json::json!({
            "agent": agent,
            "id": id,
            "error": {
                "kind": error_kind,
                "message": message,
            },
        }),
    );
}

/// Trace an infer start event.
pub fn infer_start(agent: &str, id: &str, model: &str, prompt_len: usize) {
    emit_event(
        "infer.start",
        serde_json::json!({
            "agent": agent,
            "id": id,
            "model": model,
            "prompt_len": prompt_len,
        }),
    );
}

/// Trace an infer complete event.
pub fn infer_complete(agent: &str, id: &str, model: &str, response_len: usize, duration_ms: u64) {
    emit_event(
        "infer.complete",
        serde_json::json!({
            "agent": agent,
            "id": id,
            "model": model,
            "response_len": response_len,
            "duration_ms": duration_ms,
        }),
    );
}

/// Trace an infer error event.
pub fn infer_error(agent: &str, id: &str, error_kind: &str, message: &str) {
    emit_event(
        "infer.error",
        serde_json::json!({
            "agent": agent,
            "id": id,
            "error": {
                "kind": error_kind,
                "message": message,
            },
        }),
    );
}

/// Trace a user-defined event (via the trace() keyword).
pub fn user(message: &str) {
    emit_event(
        "user",
        serde_json::json!({
            "message": message,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_ms() {
        let ts = timestamp_ms();
        // Should be a reasonable timestamp (after year 2020)
        assert!(ts > 1_577_836_800_000);
    }

    #[test]
    fn test_is_enabled_default_false() {
        // In tests, tracing should be disabled by default
        // (unless SAGE_TRACE is set in the environment)
        // We can't reliably test this without modifying global state
    }
}
