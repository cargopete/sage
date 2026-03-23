//! RFC-0011: HTTP client tool for Sage agents.
//!
//! Provides the `Http` tool with `get`, `post`, `put`, and `delete` methods.

use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

use crate::error::{SageError, SageResult};
use crate::mock::{try_get_mock, MockResponse};

/// Configuration for the HTTP client.
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// User-Agent header value.
    pub user_agent: String,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            user_agent: format!("sage-agent/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

impl HttpConfig {
    /// Create config from environment variables (native) or defaults (WASM).
    pub fn from_env() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let timeout_secs = std::env::var("SAGE_HTTP_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);

        #[cfg(target_arch = "wasm32")]
        let timeout_secs = 30;

        Self {
            timeout_secs,
            ..Default::default()
        }
    }
}

/// Response from an HTTP request.
///
/// Exposed to Sage programs as the return type of `Http.get()` etc.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HttpResponse {
    /// HTTP status code (e.g., 200, 404).
    pub status: i64,
    /// Response body as a string.
    pub body: String,
    /// Response headers.
    pub headers: HashMap<String, String>,
}

/// HTTP client for Sage agents.
///
/// Created via `HttpClient::from_env()` and used by generated code.
#[derive(Debug, Clone)]
pub struct HttpClient {
    client: reqwest::Client,
}

impl HttpClient {
    /// Create a new HTTP client with default configuration.
    pub fn new() -> Self {
        Self::with_config(HttpConfig::default())
    }

    /// Create a new HTTP client from environment variables.
    pub fn from_env() -> Self {
        Self::with_config(HttpConfig::from_env())
    }

    /// Create a new HTTP client with the given configuration.
    pub fn with_config(config: HttpConfig) -> Self {
        let builder = reqwest::Client::builder();

        // timeout() and user_agent() are not available on the WASM backend
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder
            .timeout(Duration::from_secs(config.timeout_secs))
            .user_agent(&config.user_agent);

        #[cfg(target_arch = "wasm32")]
        let _ = &config; // suppress unused warning

        let client = builder.build().expect("failed to build HTTP client");

        Self { client }
    }

    /// Perform an HTTP GET request.
    ///
    /// # Arguments
    /// * `url` - The URL to request
    ///
    /// # Returns
    /// An `HttpResponse` with status, body, and headers.
    pub async fn get(&self, url: String) -> SageResult<HttpResponse> {
        // Check for mock response first
        if let Some(mock_response) = try_get_mock("Http", "get") {
            return Self::apply_mock(mock_response);
        }

        let response = self.client.get(url).send().await?;

        let status = response.status().as_u16() as i64;
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        let body = response.text().await?;

        Ok(HttpResponse {
            status,
            body,
            headers,
        })
    }

    /// Perform an HTTP POST request.
    ///
    /// # Arguments
    /// * `url` - The URL to request
    /// * `body` - The request body as a string
    ///
    /// # Returns
    /// An `HttpResponse` with status, body, and headers.
    pub async fn post(&self, url: String, body: String) -> SageResult<HttpResponse> {
        // Check for mock response first
        if let Some(mock_response) = try_get_mock("Http", "post") {
            return Self::apply_mock(mock_response);
        }

        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        let status = response.status().as_u16() as i64;
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        let response_body = response.text().await?;

        Ok(HttpResponse {
            status,
            body: response_body,
            headers,
        })
    }

    /// Perform an HTTP PUT request.
    ///
    /// # Arguments
    /// * `url` - The URL to request
    /// * `body` - The request body as a string
    ///
    /// # Returns
    /// An `HttpResponse` with status, body, and headers.
    pub async fn put(&self, url: String, body: String) -> SageResult<HttpResponse> {
        // Check for mock response first
        if let Some(mock_response) = try_get_mock("Http", "put") {
            return Self::apply_mock(mock_response);
        }

        let response = self
            .client
            .put(url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        let status = response.status().as_u16() as i64;
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        let response_body = response.text().await?;

        Ok(HttpResponse {
            status,
            body: response_body,
            headers,
        })
    }

    /// Perform an HTTP DELETE request.
    ///
    /// # Arguments
    /// * `url` - The URL to request
    ///
    /// # Returns
    /// An `HttpResponse` with status, body, and headers.
    pub async fn delete(&self, url: String) -> SageResult<HttpResponse> {
        // Check for mock response first
        if let Some(mock_response) = try_get_mock("Http", "delete") {
            return Self::apply_mock(mock_response);
        }

        let response = self.client.delete(url).send().await?;

        let status = response.status().as_u16() as i64;
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        let body = response.text().await?;

        Ok(HttpResponse {
            status,
            body,
            headers,
        })
    }

    /// Apply a mock response, deserializing it to HttpResponse.
    fn apply_mock(mock_response: MockResponse) -> SageResult<HttpResponse> {
        match mock_response {
            MockResponse::Value(v) => serde_json::from_value(v)
                .map_err(|e| SageError::Tool(format!("mock deserialize: {e}"))),
            MockResponse::Fail(msg) => Err(SageError::Tool(msg)),
        }
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_config_defaults() {
        let config = HttpConfig::default();
        assert_eq!(config.timeout_secs, 30);
        assert!(config.user_agent.starts_with("sage-agent/"));
    }

    #[test]
    fn http_client_creates() {
        let client = HttpClient::new();
        // Just verify it doesn't panic
        drop(client);
    }

    #[tokio::test]
    async fn http_get_works() {
        // Use a mock server or skip in CI
        if std::env::var("CI").is_ok() {
            return;
        }

        let client = HttpClient::new();
        let response = client.get("https://httpbin.org/get".to_string()).await;
        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.status, 200);
        assert!(!response.body.is_empty());
    }
}
