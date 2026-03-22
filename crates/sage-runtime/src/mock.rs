//! Mock infrastructure for the Sage testing framework (RFC-0012).
//!
//! This module provides:
//! - `MockResponse` - represents either a value or error response
//! - `MockQueue` - thread-safe queue of mock responses
//! - `MockLlmClient` - mock implementation of LLM inference
//! - `MockToolRegistry` - mock implementations for tool calls
//! - Task-local mock context for tool mocking in tests

use crate::error::{SageError, SageResult};
use serde::de::DeserializeOwned;
use std::cell::RefCell;
use std::future::Future;
use std::sync::{Arc, Mutex};

// Task-local storage for the mock tool registry.
// This allows tests to intercept tool calls without threading the registry through all code.
tokio::task_local! {
    static MOCK_TOOL_REGISTRY: RefCell<Option<MockToolRegistry>>;
}

/// Run a future with a mock tool registry in scope.
///
/// All tool calls made during the execution of the future will check
/// the registry for mocks before making real calls.
///
/// # Example
/// ```ignore
/// let registry = MockToolRegistry::new();
/// registry.register("Http", "get", MockResponse::string("{\"status\": 200}"));
///
/// with_mock_tools(registry, async {
///     // Http.get() calls here will return the mock response
/// }).await;
/// ```
pub async fn with_mock_tools<F, R>(registry: MockToolRegistry, f: F) -> R
where
    F: Future<Output = R>,
{
    MOCK_TOOL_REGISTRY
        .scope(RefCell::new(Some(registry)), f)
        .await
}

/// Try to get a mock response for a tool function call.
///
/// Returns `Some(response)` if a mock is registered and available,
/// `None` if no mock is registered or if called outside a mock context.
///
/// This is called by tool clients to intercept calls during tests.
pub fn try_get_mock(tool: &str, function: &str) -> Option<MockResponse> {
    MOCK_TOOL_REGISTRY
        .try_with(|cell| {
            cell.borrow_mut()
                .as_ref()
                .and_then(|reg| reg.get(tool, function))
        })
        .ok()
        .flatten()
}

/// A mock response for an `infer` call.
#[derive(Debug, Clone)]
pub enum MockResponse {
    /// A successful response with the given value.
    Value(serde_json::Value),
    /// A failure response with the given error message.
    Fail(String),
}

impl MockResponse {
    /// Create a successful mock response from a JSON-serializable value.
    pub fn value<T: serde::Serialize>(value: T) -> Self {
        Self::Value(serde_json::to_value(value).expect("failed to serialize mock value"))
    }

    /// Create a successful mock response from a string.
    pub fn string(s: impl Into<String>) -> Self {
        Self::Value(serde_json::Value::String(s.into()))
    }

    /// Create a failure mock response.
    pub fn fail(message: impl Into<String>) -> Self {
        Self::Fail(message.into())
    }
}

/// A thread-safe queue of mock responses.
///
/// Mock responses are consumed in order - the first `infer` call gets
/// the first mock, the second gets the second, etc.
#[derive(Debug, Clone, Default)]
pub struct MockQueue {
    responses: Arc<Mutex<Vec<MockResponse>>>,
}

impl MockQueue {
    /// Create a new empty mock queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a mock queue with the given responses.
    pub fn with_responses(responses: Vec<MockResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
        }
    }

    /// Add a mock response to the queue.
    pub fn push(&self, response: MockResponse) {
        self.responses.lock().unwrap().push(response);
    }

    /// Pop the next mock response from the queue.
    ///
    /// Returns `None` if the queue is empty.
    pub fn pop(&self) -> Option<MockResponse> {
        let mut queue = self.responses.lock().unwrap();
        if queue.is_empty() {
            None
        } else {
            Some(queue.remove(0))
        }
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.responses.lock().unwrap().is_empty()
    }

    /// Get the number of remaining mock responses.
    pub fn len(&self) -> usize {
        self.responses.lock().unwrap().len()
    }
}

/// Mock LLM client for testing.
///
/// This client uses a `MockQueue` to return pre-configured responses
/// instead of making real API calls.
#[derive(Debug, Clone)]
pub struct MockLlmClient {
    queue: MockQueue,
}

impl MockLlmClient {
    /// Create a new mock client with an empty queue.
    pub fn new() -> Self {
        Self {
            queue: MockQueue::new(),
        }
    }

    /// Create a mock client with the given responses.
    pub fn with_responses(responses: Vec<MockResponse>) -> Self {
        Self {
            queue: MockQueue::with_responses(responses),
        }
    }

    /// Get a reference to the mock queue for adding responses.
    pub fn queue(&self) -> &MockQueue {
        &self.queue
    }

    /// Call the mock LLM with a prompt and return the raw string response.
    ///
    /// Returns an error if no mock responses are queued.
    pub async fn infer_string(&self, _prompt: &str) -> SageResult<String> {
        match self.queue.pop() {
            Some(MockResponse::Value(value)) => {
                // Convert JSON value to string
                match value {
                    serde_json::Value::String(s) => Ok(s),
                    other => Ok(other.to_string()),
                }
            }
            Some(MockResponse::Fail(msg)) => Err(SageError::Llm(msg)),
            None => Err(SageError::Llm(
                "infer called with no mock available (E054)".to_string(),
            )),
        }
    }

    /// Call the mock LLM with a prompt and parse the response as the given type.
    ///
    /// Returns an error if no mock responses are queued.
    pub async fn infer<T>(&self, _prompt: &str) -> SageResult<T>
    where
        T: DeserializeOwned,
    {
        match self.queue.pop() {
            Some(MockResponse::Value(value)) => serde_json::from_value(value)
                .map_err(|e| SageError::Llm(format!("failed to deserialize mock value: {e}"))),
            Some(MockResponse::Fail(msg)) => Err(SageError::Llm(msg)),
            None => Err(SageError::Llm(
                "infer called with no mock available (E054)".to_string(),
            )),
        }
    }

    /// Call the mock LLM with schema-injected prompt for structured output.
    ///
    /// Returns an error if no mock responses are queued.
    pub async fn infer_structured<T>(&self, _prompt: &str, _schema: &str) -> SageResult<T>
    where
        T: DeserializeOwned,
    {
        // Same as infer - the schema is ignored for mocks
        match self.queue.pop() {
            Some(MockResponse::Value(value)) => serde_json::from_value(value)
                .map_err(|e| SageError::Llm(format!("failed to deserialize mock value: {e}"))),
            Some(MockResponse::Fail(msg)) => Err(SageError::Llm(msg)),
            None => Err(SageError::Llm(
                "infer called with no mock available (E054)".to_string(),
            )),
        }
    }
}

impl Default for MockLlmClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock registry for tool calls.
///
/// Stores mock responses for specific tool.function combinations.
#[derive(Debug, Clone, Default)]
pub struct MockToolRegistry {
    mocks: Arc<Mutex<std::collections::HashMap<String, MockQueue>>>,
}

impl MockToolRegistry {
    /// Create a new empty mock registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a mock response for a tool function.
    ///
    /// The key is in the format "ToolName.function_name".
    pub fn register(&self, tool: &str, function: &str, response: MockResponse) {
        let key = format!("{}.{}", tool, function);
        let mut mocks = self.mocks.lock().unwrap();
        mocks.entry(key).or_default().push(response);
    }

    /// Get the next mock response for a tool function.
    ///
    /// Returns `None` if no mock is registered for this function.
    pub fn get(&self, tool: &str, function: &str) -> Option<MockResponse> {
        let key = format!("{}.{}", tool, function);
        let mocks = self.mocks.lock().unwrap();
        mocks.get(&key).and_then(|q| q.pop())
    }

    /// Check if a mock is registered for a tool function.
    pub fn has_mock(&self, tool: &str, function: &str) -> bool {
        let key = format!("{}.{}", tool, function);
        let mocks = self.mocks.lock().unwrap();
        mocks.get(&key).is_some_and(|q| !q.is_empty())
    }

    /// Call a mocked tool function and return the result.
    ///
    /// Returns an error if no mock is registered.
    pub async fn call<T>(&self, tool: &str, function: &str) -> SageResult<T>
    where
        T: DeserializeOwned,
    {
        match self.get(tool, function) {
            Some(MockResponse::Value(value)) => serde_json::from_value(value).map_err(|e| {
                SageError::Tool(format!("failed to deserialize mock tool response: {e}"))
            }),
            Some(MockResponse::Fail(msg)) => Err(SageError::Tool(msg)),
            None => Err(SageError::Tool(format!(
                "no mock registered for {}.{}",
                tool, function
            ))),
        }
    }

    /// Call a mocked tool function and return the raw string.
    pub async fn call_string(&self, tool: &str, function: &str) -> SageResult<String> {
        match self.get(tool, function) {
            Some(MockResponse::Value(value)) => match value {
                serde_json::Value::String(s) => Ok(s),
                other => Ok(other.to_string()),
            },
            Some(MockResponse::Fail(msg)) => Err(SageError::Tool(msg)),
            None => Err(SageError::Tool(format!(
                "no mock registered for {}.{}",
                tool, function
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_infer_string_returns_value() {
        let client = MockLlmClient::with_responses(vec![MockResponse::string("hello world")]);
        let result = client.infer_string("test").await.unwrap();
        assert_eq!(result, "hello world");
    }

    #[tokio::test]
    async fn mock_infer_string_returns_fail() {
        let client = MockLlmClient::with_responses(vec![MockResponse::fail("test error")]);
        let result = client.infer_string("test").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("test error"));
    }

    #[tokio::test]
    async fn mock_infer_empty_queue_returns_error() {
        let client = MockLlmClient::new();
        let result = client.infer_string("test").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("E054"));
    }

    #[tokio::test]
    async fn mock_queue_fifo_order() {
        let client = MockLlmClient::with_responses(vec![
            MockResponse::string("first"),
            MockResponse::string("second"),
            MockResponse::string("third"),
        ]);

        assert_eq!(client.infer_string("a").await.unwrap(), "first");
        assert_eq!(client.infer_string("b").await.unwrap(), "second");
        assert_eq!(client.infer_string("c").await.unwrap(), "third");
        assert!(client.infer_string("d").await.is_err());
    }

    #[tokio::test]
    async fn mock_infer_typed_value() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Person {
            name: String,
            age: i32,
        }

        let client = MockLlmClient::with_responses(vec![MockResponse::value(
            serde_json::json!({ "name": "Ward", "age": 42 }),
        )]);

        let person: Person = client.infer("test").await.unwrap();
        assert_eq!(person.name, "Ward");
        assert_eq!(person.age, 42);
    }

    #[test]
    fn mock_queue_thread_safe() {
        use std::thread;

        let queue = MockQueue::with_responses(vec![
            MockResponse::string("1"),
            MockResponse::string("2"),
            MockResponse::string("3"),
        ]);

        let queue_clone = queue.clone();
        let handle = thread::spawn(move || {
            queue_clone.pop();
            queue_clone.pop();
        });

        handle.join().unwrap();
        assert_eq!(queue.len(), 1);
    }

    #[tokio::test]
    async fn mock_infer_structured() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct Summary {
            text: String,
            confidence: f64,
        }

        let client = MockLlmClient::with_responses(vec![MockResponse::value(serde_json::json!({
            "text": "A summary",
            "confidence": 0.95
        }))]);

        let summary: Summary = client
            .infer_structured("summarize", "schema")
            .await
            .unwrap();
        assert_eq!(summary.text, "A summary");
        assert!((summary.confidence - 0.95).abs() < 0.001);
    }

    #[tokio::test]
    async fn mock_tool_registry_basic() {
        let registry = MockToolRegistry::new();

        // Register a mock
        registry.register("Http", "get", MockResponse::string("mocked response"));

        // Should have mock
        assert!(registry.has_mock("Http", "get"));

        // Call and get result
        let result: String = registry.call("Http", "get").await.unwrap();
        assert_eq!(result, "mocked response");

        // Queue should be empty now
        assert!(!registry.has_mock("Http", "get"));
    }

    #[tokio::test]
    async fn mock_tool_registry_multiple() {
        let registry = MockToolRegistry::new();

        // Register multiple mocks for same function
        registry.register("Http", "get", MockResponse::string("first"));
        registry.register("Http", "get", MockResponse::string("second"));

        // Should get them in order
        let r1: String = registry.call("Http", "get").await.unwrap();
        let r2: String = registry.call("Http", "get").await.unwrap();

        assert_eq!(r1, "first");
        assert_eq!(r2, "second");
    }

    #[tokio::test]
    async fn mock_tool_registry_fail() {
        let registry = MockToolRegistry::new();
        registry.register("Http", "get", MockResponse::fail("network error"));

        let result: Result<String, _> = registry.call("Http", "get").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("network error"));
    }

    #[tokio::test]
    async fn mock_tool_registry_no_mock() {
        let registry = MockToolRegistry::new();

        let result: Result<String, _> = registry.call("Http", "get").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("no mock registered"));
    }
}
