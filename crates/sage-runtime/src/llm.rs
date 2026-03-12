//! LLM client for inference calls.

use crate::error::{SageError, SageResult};
use serde::{Deserialize, Serialize};

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
}

impl LlmConfig {
    /// Create a config from environment variables.
    pub fn from_env() -> Self {
        Self {
            api_key: std::env::var("SAGE_API_KEY").unwrap_or_default(),
            base_url: std::env::var("SAGE_LLM_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            model: std::env::var("SAGE_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string()),
        }
    }

    /// Create a mock config for testing.
    pub fn mock() -> Self {
        Self {
            api_key: "mock".to_string(),
            base_url: "mock".to_string(),
            model: "mock".to_string(),
        }
    }

    /// Check if this is a mock configuration.
    pub fn is_mock(&self) -> bool {
        self.api_key == "mock"
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

        let request = ChatRequest {
            model: &self.config.model,
            messages: vec![ChatMessage {
                role: "user",
                content: prompt,
            }],
        };

        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
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

    /// Call the LLM with a prompt and parse the response as the given type.
    pub async fn infer<T>(&self, prompt: &str) -> SageResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let response = self.infer_string(prompt).await?;

        // Try to parse as JSON first
        if let Ok(value) = serde_json::from_str(&response) {
            return Ok(value);
        }

        // Try to parse as JSON, stripping markdown code blocks if present
        let cleaned = response
            .trim()
            .strip_prefix("```json")
            .unwrap_or(&response)
            .strip_prefix("```")
            .unwrap_or(&response)
            .strip_suffix("```")
            .unwrap_or(&response)
            .trim();

        serde_json::from_str(cleaned).map_err(|e| {
            SageError::Llm(format!(
                "Failed to parse LLM response as {}: {e}\nResponse: {response}",
                std::any::type_name::<T>()
            ))
        })
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
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
}
