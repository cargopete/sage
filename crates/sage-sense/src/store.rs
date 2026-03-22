//! Document state management for the LSP server.

use std::collections::HashMap;
use std::sync::Mutex;
use tower_lsp::lsp_types::{Diagnostic, Url};

/// State for a single open document.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Document {
    /// The document's text content.
    pub text: String,
    /// Current diagnostics for this document.
    pub diagnostics: Vec<Diagnostic>,
    /// Document version from the editor.
    pub version: i32,
}

/// Thread-safe store of all open documents.
pub struct DocumentStore {
    inner: Mutex<HashMap<String, Document>>,
}

#[allow(dead_code)]
impl DocumentStore {
    /// Create a new empty document store.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Update a document with new text and diagnostics.
    pub fn update(&self, uri: Url, text: String, diagnostics: Vec<Diagnostic>, version: i32) {
        let mut map = self.inner.lock().unwrap();
        map.insert(
            uri.to_string(),
            Document {
                text,
                diagnostics,
                version,
            },
        );
    }

    /// Remove a document from the store.
    pub fn remove(&self, uri: &Url) {
        self.inner.lock().unwrap().remove(&uri.to_string());
    }

    /// Get the diagnostics for a document.
    pub fn get_diagnostics(&self, uri: &Url) -> Vec<Diagnostic> {
        self.inner
            .lock()
            .unwrap()
            .get(&uri.to_string())
            .map(|d| d.diagnostics.clone())
            .unwrap_or_default()
    }

    /// Get the text content of a document.
    pub fn get_text(&self, uri: &Url) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .get(&uri.to_string())
            .map(|d| d.text.clone())
    }
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self::new()
    }
}
