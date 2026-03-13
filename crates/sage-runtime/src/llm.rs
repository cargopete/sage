//! LLM client for inference calls.

use crate::error::{SageError, SageResult};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Default number of retries for structured inference.
const DEFAULT_INFER_RETRIES: usize = 3;

/// Client for making LLM inference calls.
#[derive(Clone)]
pub struct LlmClient {
    client: reqwest::Client,
    config: LlmConfig,
}

/// Configuration for the LLM client.
#[derive(Clone)]
pub struct LlmConfig {
    /// API key for authentication.
    pub api_key: String,
    /// Base URL for the API.
    pub base_url: String,
    /// Model to use.
    pub model: String,
    /// Max retries for structured inference.
    pub infer_retries: usize,
}

impl LlmConfig {
    /// Create a config from environment variables.
    pub fn from_env() -> Self {
        Self {
            api_key: std::env::var("SAGE_API_KEY").unwrap_or_default(),
            base_url: std::env::var("SAGE_LLM_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            model: std::env::var("SAGE_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string()),
            infer_retries: std::env::var("SAGE_INFER_RETRIES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_INFER_RETRIES),
        }
    }

    /// Create a mock config for testing.
    pub fn mock() -> Self {
        Self {
            api_key: "mock".to_string(),
            base_url: "mock".to_string(),
            model: "mock".to_string(),
            infer_retries: DEFAULT_INFER_RETRIES,
        }
    }

    /// Check if this is a mock configuration.
    pub fn is_mock(&self) -> bool {
        self.api_key == "mock"
    }

    /// Check if the base URL points to a local Ollama instance.
    pub fn is_ollama(&self) -> bool {
        self.base_url.contains("localhost") || self.base_url.contains("127.0.0.1")
    }
}

impl LlmClient {
    /// Create a new LLM client with the given configuration.
    pub fn new(config: LlmConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }

    /// Create a client from environment variables.
    pub fn from_env() -> Self {
        Self::new(LlmConfig::from_env())
    }

    /// Create a mock client for testing.
    pub fn mock() -> Self {
        Self::new(LlmConfig::mock())
    }

    /// Call the LLM with a prompt and return the raw string response.
    pub async fn infer_string(&self, prompt: &str) -> SageResult<String> {
        if self.config.is_mock() {
            return Ok(format!("[Mock LLM response for: {prompt}]"));
        }

        let request = ChatRequest::new(
            &self.config.model,
            vec![ChatMessage {
                role: "user",
                content: prompt,
            }],
        );

        self.send_request(&request).await
    }

    /// Call the LLM with a prompt and parse the response as the given type.
    pub async fn infer<T>(&self, prompt: &str) -> SageResult<T>
    where
        T: DeserializeOwned,
    {
        let response = self.infer_string(prompt).await?;
        parse_json_response(&response)
    }

    /// Call the LLM with schema-injected prompt engineering for structured output.
    ///
    /// The schema is injected as a system message, and the runtime retries up to
    /// `SAGE_INFER_RETRIES` times (default 3) on parse failure.
    pub async fn infer_structured<T>(&self, prompt: &str, schema: &str) -> SageResult<T>
    where
        T: DeserializeOwned,
    {
        if self.config.is_mock() {
            // For mock mode, return an error since we can't produce valid structured output
            return Err(SageError::Llm(
                "Mock client cannot produce structured output".to_string(),
            ));
        }

        let system_prompt = format!(
            "You are a precise assistant that always responds with valid JSON.\n\
             You must respond with a JSON object matching this exact schema:\n\n\
             {schema}\n\n\
             Respond with JSON only. No explanation, no markdown, no code blocks."
        );

        let mut last_error: Option<String> = None;

        for attempt in 0..self.config.infer_retries {
            let response = if attempt == 0 {
                self.send_structured_request(&system_prompt, prompt, None)
                    .await?
            } else {
                let error_feedback = format!(
                    "Your previous response could not be parsed: {}\n\
                     Please try again, responding with valid JSON only.",
                    last_error.as_deref().unwrap_or("unknown error")
                );
                self.send_structured_request(&system_prompt, prompt, Some(&error_feedback))
                    .await?
            };

            match parse_json_response::<T>(&response) {
                Ok(value) => return Ok(value),
                Err(e) => {
                    last_error = Some(e.to_string());
                    // Continue to next retry
                }
            }
        }

        Err(SageError::Llm(format!(
            "Failed to parse structured response after {} attempts: {}",
            self.config.infer_retries,
            last_error.unwrap_or_else(|| "unknown error".to_string())
        )))
    }

    /// Send a structured inference request with optional error feedback.
    async fn send_structured_request(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        error_feedback: Option<&str>,
    ) -> SageResult<String> {
        let mut messages = vec![
            ChatMessage {
                role: "system",
                content: system_prompt,
            },
            ChatMessage {
                role: "user",
                content: user_prompt,
            },
        ];

        if let Some(feedback) = error_feedback {
            messages.push(ChatMessage {
                role: "user",
                content: feedback,
            });
        }

        let mut request = ChatRequest::new(&self.config.model, messages);

        // Add format: json hint for Ollama
        if self.config.is_ollama() {
            request = request.with_json_format();
        }

        self.send_request(&request).await
    }

    /// Send a chat request and return the response content.
    async fn send_request(&self, request: &ChatRequest<'_>) -> SageResult<String> {
        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(SageError::Llm(format!("API error {status}: {body}")));
        }

        let chat_response: ChatResponse = response.json().await?;
        let content = chat_response
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default();

        Ok(content)
    }
}

/// Strip markdown code fences from a response and parse as JSON.
fn parse_json_response<T: DeserializeOwned>(response: &str) -> SageResult<T> {
    // Try to parse as-is first
    if let Ok(value) = serde_json::from_str(response) {
        return Ok(value);
    }

    // Strip markdown code blocks if present
    let cleaned = response
        .trim()
        .strip_prefix("```json")
        .or_else(|| response.trim().strip_prefix("```"))
        .unwrap_or(response.trim());

    let cleaned = cleaned.strip_suffix("```").unwrap_or(cleaned).trim();

    serde_json::from_str(cleaned).map_err(|e| {
        SageError::Llm(format!(
            "Failed to parse LLM response as {}: {e}\nResponse: {response}",
            std::any::type_name::<T>()
        ))
    })
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<&'a str>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

impl<'a> ChatRequest<'a> {
    fn new(model: &'a str, messages: Vec<ChatMessage<'a>>) -> Self {
        Self {
            model,
            messages,
            format: None,
        }
    }

    fn with_json_format(mut self) -> Self {
        self.format = Some("json");
        self
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_client_returns_placeholder() {
        let client = LlmClient::mock();
        let response = client.infer_string("test prompt").await.unwrap();
        assert!(response.contains("Mock LLM response"));
        assert!(response.contains("test prompt"));
    }

    #[test]
    fn parse_json_strips_markdown_fences() {
        let response = "```json\n{\"value\": 42}\n```";
        let result: serde_json::Value = parse_json_response(response).unwrap();
        assert_eq!(result["value"], 42);
    }

    #[test]
    fn parse_json_handles_plain_json() {
        let response = r#"{"name": "test"}"#;
        let result: serde_json::Value = parse_json_response(response).unwrap();
        assert_eq!(result["name"], "test");
    }

    #[test]
    fn parse_json_handles_generic_code_block() {
        let response = "```\n{\"x\": 1}\n```";
        let result: serde_json::Value = parse_json_response(response).unwrap();
        assert_eq!(result["x"], 1);
    }

    #[test]
    fn ollama_detection_localhost() {
        let config = LlmConfig {
            api_key: "test".to_string(),
            base_url: "http://localhost:11434/v1".to_string(),
            model: "llama2".to_string(),
            infer_retries: 3,
        };
        assert!(config.is_ollama());
    }

    #[test]
    fn ollama_detection_127() {
        let config = LlmConfig {
            api_key: "test".to_string(),
            base_url: "http://127.0.0.1:11434/v1".to_string(),
            model: "llama2".to_string(),
            infer_retries: 3,
        };
        assert!(config.is_ollama());
    }

    #[test]
    fn not_ollama_for_openai() {
        let config = LlmConfig {
            api_key: "test".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4".to_string(),
            infer_retries: 3,
        };
        assert!(!config.is_ollama());
    }

    #[test]
    fn chat_request_json_format() {
        let request = ChatRequest::new("model", vec![]).with_json_format();
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains(r#""format":"json""#));
    }

    #[test]
    fn chat_request_no_format_by_default() {
        let request = ChatRequest::new("model", vec![]);
        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains("format"));
    }

    #[tokio::test]
    async fn infer_structured_fails_on_mock() {
        let client = LlmClient::mock();
        let result: Result<serde_json::Value, _> = client.infer_structured("test", "{}").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Mock client"));
    }
}
