//! WASM platform layer for the Sage runtime.
//!
//! Provides browser-compatible implementations of platform primitives:
//! - `spawn_local` for async task spawning (no `Send` required)
//! - `sleep` via browser `setTimeout`
//! - Console logging via `web-sys`
//! - `web-time` re-export for `Instant` shim
//! - Thread-local LLM config injection for browser environments

#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::time::Duration;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

/// Re-export `web_time::Instant` as the WASM-compatible `Instant`.
pub use web_time::Instant;

/// Re-export `wasm_bindgen_futures::spawn_local`.
pub use wasm_bindgen_futures::spawn_local;

/// Re-export panic hook setup.
pub use console_error_panic_hook::set_once as set_panic_hook;

/// LLM configuration for WASM environments.
///
/// Since `std::env::var` is not available in the browser, LLM config
/// is injected via `sage_configure()` from JavaScript.
#[derive(Debug, Clone)]
pub struct WasmLlmConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl Default for WasmLlmConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
        }
    }
}

thread_local! {
    static WASM_LLM_CONFIG: RefCell<WasmLlmConfig> = RefCell::new(WasmLlmConfig::default());
}

/// Configure the LLM endpoint from JavaScript.
///
/// Call this before starting the Sage agent:
/// ```js
/// import init, { sage_configure } from './pkg/agent.js';
/// await init();
/// sage_configure('https://my-proxy.example.com/v1', 'gpt-4o', '');
/// ```
#[wasm_bindgen]
pub fn sage_configure(base_url: &str, model: &str, api_key: &str) {
    WASM_LLM_CONFIG.with(|c| {
        *c.borrow_mut() = WasmLlmConfig {
            api_key: api_key.to_string(),
            base_url: base_url.to_string(),
            model: model.to_string(),
        };
    });
}

/// Get the current WASM LLM configuration.
pub fn get_llm_config() -> WasmLlmConfig {
    WASM_LLM_CONFIG.with(|c| c.borrow().clone())
}

/// Async sleep using browser `setTimeout`.
pub async fn sleep(duration: Duration) {
    let ms = duration.as_millis() as i32;
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        // Use global scope setTimeout (works in both Window and Worker contexts)
        let global = js_sys::global();
        let _ = js_sys::Reflect::apply(
            &js_sys::Function::from(js_sys::Reflect::get(&global, &"setTimeout".into()).unwrap()),
            &global,
            &js_sys::Array::of2(&resolve, &JsValue::from(ms)),
        );
    });
    let _ = JsFuture::from(promise).await;
}

/// Log a message to the browser console.
pub fn console_log(msg: &str) {
    web_sys::console::log_1(&msg.into());
}

/// Log a warning to the browser console.
pub fn console_warn(msg: &str) {
    web_sys::console::warn_1(&msg.into());
}

/// Log an error to the browser console.
pub fn console_error(msg: &str) {
    web_sys::console::error_1(&msg.into());
}
