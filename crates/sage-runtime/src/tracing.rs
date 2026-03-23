//! Tracing support for Sage programs.
//!
//! Supports multiple backends:
//! - `ndjson`: Newline-delimited JSON to stderr or file (default, native only)
//! - `otlp`: OpenTelemetry Protocol HTTP/JSON export (native only)
//! - `console`: Browser console output (WASM only)
//! - `none`: Tracing disabled

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::AtomicU64;
use std::time::{SystemTime, UNIX_EPOCH};

/// Global tracing state.
static TRACER: OnceLock<Arc<Tracer>> = OnceLock::new();

/// Check if tracing is enabled.
#[inline]
pub fn is_enabled() -> bool {
    TRACER
        .get()
        .map(|t| t.enabled.load(Ordering::Relaxed))
        .unwrap_or(false)
}

/// Configuration for the tracing backend.
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Backend type: "ndjson", "otlp", "console", or "none".
    pub backend: String,
    /// OTLP endpoint URL (for otlp backend).
    pub otlp_endpoint: Option<String>,
    /// Service name for trace attribution.
    pub service_name: String,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            backend: "ndjson".to_string(),
            #[cfg(target_arch = "wasm32")]
            backend: "console".to_string(),
            otlp_endpoint: None,
            service_name: "sage-agent".to_string(),
        }
    }
}

/// Initialize tracing with the given configuration.
pub fn init_with_config(config: TracingConfig) {
    let tracer = match config.backend.as_str() {
        "none" => Tracer::disabled(),
        #[cfg(not(target_arch = "wasm32"))]
        "otlp" => {
            let endpoint = config
                .otlp_endpoint
                .unwrap_or_else(|| "http://localhost:4318/v1/traces".to_string());
            Tracer::otlp(endpoint, config.service_name)
        }
        #[cfg(target_arch = "wasm32")]
        "console" => Tracer::console(),
        #[cfg(not(target_arch = "wasm32"))]
        _ => {
            // Check environment variables for NDJSON output
            if let Ok(path) = std::env::var("SAGE_TRACE_FILE") {
                match std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                {
                    Ok(file) => Tracer::ndjson_file(file),
                    Err(e) => {
                        eprintln!("Warning: Could not open trace file {}: {}", path, e);
                        Tracer::ndjson_stderr()
                    }
                }
            } else if std::env::var("SAGE_TRACE").is_ok() {
                Tracer::ndjson_stderr()
            } else {
                Tracer::disabled()
            }
        }
        #[cfg(target_arch = "wasm32")]
        _ => Tracer::console(),
    };

    let _ = TRACER.set(Arc::new(tracer));
}

/// Initialize tracing from environment variables (legacy compatibility).
pub fn init() {
    init_with_config(TracingConfig::default());
}

/// Get current timestamp in milliseconds.
fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Get current timestamp in nanoseconds.
#[cfg(not(target_arch = "wasm32"))]
fn timestamp_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Generate a random trace ID (16 bytes as hex).
#[cfg(not(target_arch = "wasm32"))]
fn generate_trace_id() -> String {
    use std::time::Instant;
    let now = Instant::now();
    let seed = now.elapsed().as_nanos() as u64;
    format!("{:032x}", seed ^ timestamp_ns())
}

/// Generate a random span ID (8 bytes as hex).
#[cfg(not(target_arch = "wasm32"))]
fn generate_span_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{:016x}", count ^ (timestamp_ns() & 0xFFFF_FFFF))
}

/// Tracing backend implementation.
struct Tracer {
    enabled: AtomicBool,
    backend: Mutex<TracerBackend>,
    #[cfg(not(target_arch = "wasm32"))]
    service_name: String,
    #[cfg(not(target_arch = "wasm32"))]
    trace_id: String,
}

enum TracerBackend {
    Disabled,
    #[cfg(not(target_arch = "wasm32"))]
    Ndjson(NdjsonBackend),
    #[cfg(not(target_arch = "wasm32"))]
    Otlp(OtlpBackend),
    #[cfg(target_arch = "wasm32")]
    Console,
}

impl Tracer {
    fn disabled() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            backend: Mutex::new(TracerBackend::Disabled),
            #[cfg(not(target_arch = "wasm32"))]
            service_name: "sage-agent".to_string(),
            #[cfg(not(target_arch = "wasm32"))]
            trace_id: generate_trace_id(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn ndjson_stderr() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            backend: Mutex::new(TracerBackend::Ndjson(NdjsonBackend::Stderr)),
            service_name: "sage-agent".to_string(),
            trace_id: generate_trace_id(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn ndjson_file(file: std::fs::File) -> Self {
        Self {
            enabled: AtomicBool::new(true),
            backend: Mutex::new(TracerBackend::Ndjson(NdjsonBackend::File(file))),
            service_name: "sage-agent".to_string(),
            trace_id: generate_trace_id(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn otlp(endpoint: String, service_name: String) -> Self {
        Self {
            enabled: AtomicBool::new(true),
            backend: Mutex::new(TracerBackend::Otlp(OtlpBackend::new(endpoint))),
            service_name,
            trace_id: generate_trace_id(),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn console() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            backend: Mutex::new(TracerBackend::Console),
        }
    }

    fn emit(&self, kind: &str, data: serde_json::Value) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        let mut backend = self.backend.lock().unwrap();
        match &mut *backend {
            TracerBackend::Disabled => {}
            #[cfg(not(target_arch = "wasm32"))]
            TracerBackend::Ndjson(ndjson) => {
                ndjson.emit(kind, data);
            }
            #[cfg(not(target_arch = "wasm32"))]
            TracerBackend::Otlp(otlp) => {
                otlp.emit(kind, data, &self.trace_id, &self.service_name);
            }
            #[cfg(target_arch = "wasm32")]
            TracerBackend::Console => {
                if let Ok(json) = serde_json::to_string(&serde_json::json!({
                    "t": timestamp_ms(),
                    "kind": kind,
                    "data": data,
                })) {
                    sage_runtime_web::console_log(&json);
                }
            }
        }
    }
}

// ===========================================================================
// Native-only backends
// ===========================================================================

#[cfg(not(target_arch = "wasm32"))]
mod native_backends {
    use super::*;
    use serde::Serialize;
    use std::io::Write;

    /// NDJSON backend for local trace output.
    pub(super) enum NdjsonBackend {
        Stderr,
        File(std::fs::File),
    }

    impl NdjsonBackend {
        pub(super) fn emit(&mut self, kind: &str, data: serde_json::Value) {
            #[derive(Serialize)]
            struct TraceEvent<'a> {
                t: u64,
                kind: &'a str,
                #[serde(flatten)]
                data: serde_json::Value,
            }

            let event = TraceEvent {
                t: timestamp_ms(),
                kind,
                data,
            };

            if let Ok(json) = serde_json::to_string(&event) {
                let line = format!("{}\n", json);
                match self {
                    NdjsonBackend::Stderr => {
                        let _ = std::io::stderr().write_all(line.as_bytes());
                    }
                    NdjsonBackend::File(f) => {
                        let _ = f.write_all(line.as_bytes());
                    }
                }
            }
        }
    }

    /// OTLP HTTP/JSON backend for OpenTelemetry export.
    pub(super) struct OtlpBackend {
        endpoint: String,
        pending_spans: Vec<OtlpSpan>,
    }

    impl OtlpBackend {
        pub(super) fn new(endpoint: String) -> Self {
            Self {
                endpoint,
                pending_spans: Vec::new(),
            }
        }

        pub(super) fn emit(
            &mut self,
            kind: &str,
            data: serde_json::Value,
            trace_id: &str,
            service_name: &str,
        ) {
            let span_id = generate_span_id();
            let now_ns = timestamp_ns();

            let span = OtlpSpan {
                trace_id: trace_id.to_string(),
                span_id,
                name: kind.to_string(),
                kind: 1, // INTERNAL
                start_time_unix_nano: now_ns,
                end_time_unix_nano: now_ns,
                attributes: data_to_attributes(&data),
                status: OtlpStatus { code: 1 }, // OK
            };

            self.pending_spans.push(span);

            if self.pending_spans.len() >= 10
                || kind.contains("stop")
                || kind.contains("error")
            {
                self.flush(service_name);
            }
        }

        fn flush(&mut self, service_name: &str) {
            if self.pending_spans.is_empty() {
                return;
            }

            let spans = std::mem::take(&mut self.pending_spans);
            let payload = OtlpExportRequest {
                resource_spans: vec![OtlpResourceSpans {
                    resource: OtlpResource {
                        attributes: vec![OtlpAttribute {
                            key: "service.name".to_string(),
                            value: OtlpValue {
                                string_value: Some(service_name.to_string()),
                            },
                        }],
                    },
                    scope_spans: vec![OtlpScopeSpans {
                        scope: OtlpScope {
                            name: "sage".to_string(),
                            version: env!("CARGO_PKG_VERSION").to_string(),
                        },
                        spans,
                    }],
                }],
            };

            let endpoint = self.endpoint.clone();
            if let Ok(json) = serde_json::to_string(&payload) {
                std::thread::spawn(move || {
                    let _ = ureq_post(&endpoint, &json);
                });
            }
        }
    }

    /// Simple blocking HTTP POST (used in background thread).
    fn ureq_post(url: &str, body: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::Read;
        use std::net::TcpStream;

        let url = url.trim_start_matches("http://");
        let (host_port, path) = url.split_once('/').unwrap_or((url, "v1/traces"));
        let path = format!("/{}", path);

        let mut stream = TcpStream::connect(host_port)?;
        stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;

        let request = format!(
            "POST {} HTTP/1.1\r\n\
             Host: {}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {}",
            path,
            host_port,
            body.len(),
            body
        );

        stream.write_all(request.as_bytes())?;

        let mut response = Vec::new();
        let _ = stream.read_to_end(&mut response);

        Ok(())
    }

    /// Convert JSON data to OTLP attributes.
    fn data_to_attributes(data: &serde_json::Value) -> Vec<OtlpAttribute> {
        let mut attrs = Vec::new();

        if let serde_json::Value::Object(map) = data {
            for (key, value) in map {
                let attr = match value {
                    serde_json::Value::String(s) => OtlpAttribute {
                        key: key.clone(),
                        value: OtlpValue {
                            string_value: Some(s.clone()),
                        },
                    },
                    serde_json::Value::Number(n) => OtlpAttribute {
                        key: key.clone(),
                        value: OtlpValue {
                            string_value: Some(n.to_string()),
                        },
                    },
                    serde_json::Value::Bool(b) => OtlpAttribute {
                        key: key.clone(),
                        value: OtlpValue {
                            string_value: Some(b.to_string()),
                        },
                    },
                    _ => OtlpAttribute {
                        key: key.clone(),
                        value: OtlpValue {
                            string_value: Some(value.to_string()),
                        },
                    },
                };
                attrs.push(attr);
            }
        }

        attrs
    }

    // OTLP JSON structures

    #[derive(Serialize)]
    pub(super) struct OtlpExportRequest {
        #[serde(rename = "resourceSpans")]
        pub resource_spans: Vec<OtlpResourceSpans>,
    }

    #[derive(Serialize)]
    pub(super) struct OtlpResourceSpans {
        pub resource: OtlpResource,
        #[serde(rename = "scopeSpans")]
        pub scope_spans: Vec<OtlpScopeSpans>,
    }

    #[derive(Serialize)]
    pub(super) struct OtlpResource {
        pub attributes: Vec<OtlpAttribute>,
    }

    #[derive(Serialize)]
    pub(super) struct OtlpScopeSpans {
        pub scope: OtlpScope,
        pub spans: Vec<OtlpSpan>,
    }

    #[derive(Serialize)]
    pub(super) struct OtlpScope {
        pub name: String,
        pub version: String,
    }

    #[derive(Serialize)]
    pub(super) struct OtlpSpan {
        #[serde(rename = "traceId")]
        pub trace_id: String,
        #[serde(rename = "spanId")]
        pub span_id: String,
        pub name: String,
        pub kind: i32,
        #[serde(rename = "startTimeUnixNano")]
        pub start_time_unix_nano: u64,
        #[serde(rename = "endTimeUnixNano")]
        pub end_time_unix_nano: u64,
        pub attributes: Vec<OtlpAttribute>,
        pub status: OtlpStatus,
    }

    #[derive(Serialize)]
    pub(super) struct OtlpAttribute {
        pub key: String,
        pub value: OtlpValue,
    }

    #[derive(Serialize)]
    pub(super) struct OtlpValue {
        #[serde(rename = "stringValue", skip_serializing_if = "Option::is_none")]
        pub string_value: Option<String>,
    }

    #[derive(Serialize)]
    pub(super) struct OtlpStatus {
        pub code: i32,
    }
}

#[cfg(not(target_arch = "wasm32"))]
use native_backends::{NdjsonBackend, OtlpBackend};

// ===========================================================================
// Public API functions
// ===========================================================================

fn emit_event(kind: &str, data: serde_json::Value) {
    if let Some(tracer) = TRACER.get() {
        tracer.emit(kind, data);
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

/// Trace the start of a span block.
pub fn span_start(name: &str) {
    emit_event(
        "span.start",
        serde_json::json!({
            "name": name,
        }),
    );
}

/// Trace the end of a span block with duration.
pub fn span_end(name: &str, duration_ms: u64) {
    emit_event(
        "span.end",
        serde_json::json!({
            "name": name,
            "duration_ms": duration_ms,
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
    fn test_timestamp_ns() {
        let ts = timestamp_ns();
        // Should be a reasonable timestamp in nanoseconds
        assert!(ts > 1_577_836_800_000_000_000);
    }

    #[test]
    fn test_generate_trace_id() {
        let id1 = generate_trace_id();
        let id2 = generate_trace_id();
        assert_eq!(id1.len(), 32);
        assert_eq!(id2.len(), 32);
    }

    #[test]
    fn test_generate_span_id() {
        let id1 = generate_span_id();
        let id2 = generate_span_id();
        assert_eq!(id1.len(), 16);
        assert_eq!(id2.len(), 16);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_tracing_config_default() {
        let config = TracingConfig::default();
        assert_eq!(config.backend, "ndjson");
        assert!(config.otlp_endpoint.is_none());
        assert_eq!(config.service_name, "sage-agent");
    }
}
