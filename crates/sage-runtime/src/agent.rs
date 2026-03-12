//! Agent spawning and lifecycle management.

use crate::error::{SageError, SageResult};
use crate::llm::LlmClient;
use std::future::Future;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// Handle to a spawned agent.
///
/// This is returned by `spawn()` and can be awaited to get the agent's result.
pub struct AgentHandle<T> {
    join: JoinHandle<SageResult<T>>,
    #[allow(dead_code)]
    message_tx: mpsc::Sender<Message>,
}

impl<T> AgentHandle<T> {
    /// Wait for the agent to complete and return its result.
    pub async fn result(self) -> SageResult<T> {
        self.join.await?
    }

    /// Send a message to the agent.
    #[allow(dead_code)]
    pub async fn send(&self, msg: Message) -> SageResult<()> {
        self.message_tx
            .send(msg)
            .await
            .map_err(|e| SageError::Agent(format!("Failed to send message: {e}")))
    }
}

/// A message that can be sent to an agent.
#[derive(Debug, Clone)]
pub struct Message {
    /// The message payload as a JSON value.
    pub payload: serde_json::Value,
}

impl Message {
    /// Create a new message from a serializable value.
    pub fn new<T: serde::Serialize>(value: T) -> SageResult<Self> {
        Ok(Self {
            payload: serde_json::to_value(value)?,
        })
    }
}

/// Context provided to agent handlers.
///
/// This gives agents access to LLM inference and the ability to emit results.
pub struct AgentContext<T> {
    /// LLM client for inference calls.
    pub llm: LlmClient,
    /// Channel to send the result to the awaiter.
    result_tx: Option<oneshot::Sender<T>>,
    /// Channel to receive messages.
    #[allow(dead_code)]
    message_rx: mpsc::Receiver<Message>,
}

impl<T> AgentContext<T> {
    /// Create a new agent context.
    fn new(
        llm: LlmClient,
        result_tx: oneshot::Sender<T>,
        message_rx: mpsc::Receiver<Message>,
    ) -> Self {
        Self {
            llm,
            result_tx: Some(result_tx),
            message_rx,
        }
    }

    /// Emit a value to the awaiter.
    ///
    /// This should be called once at the end of the agent's execution.
    pub fn emit(mut self, value: T) -> SageResult<T>
    where
        T: Clone,
    {
        if let Some(tx) = self.result_tx.take() {
            // Ignore send errors - the receiver may have been dropped
            let _ = tx.send(value.clone());
        }
        Ok(value)
    }

    /// Call the LLM with a prompt and parse the response.
    pub async fn infer<R>(&self, prompt: &str) -> SageResult<R>
    where
        R: serde::de::DeserializeOwned,
    {
        self.llm.infer(prompt).await
    }

    /// Call the LLM with a prompt and return the raw string response.
    pub async fn infer_string(&self, prompt: &str) -> SageResult<String> {
        self.llm.infer_string(prompt).await
    }
}

/// Spawn an agent and return a handle to it.
///
/// The agent will run asynchronously in a separate task.
pub fn spawn<A, T, F>(agent: A) -> AgentHandle<T>
where
    A: FnOnce(AgentContext<T>) -> F + Send + 'static,
    F: Future<Output = SageResult<T>> + Send,
    T: Send + 'static,
{
    let (result_tx, result_rx) = oneshot::channel();
    let (message_tx, message_rx) = mpsc::channel(32);

    let llm = LlmClient::from_env();
    let ctx = AgentContext::new(llm, result_tx, message_rx);

    let join = tokio::spawn(async move { agent(ctx).await });

    // We need to handle the result_rx somewhere, but for now we just let
    // the result come from the JoinHandle
    drop(result_rx);

    AgentHandle { join, message_tx }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spawn_simple_agent() {
        let handle = spawn(|ctx: AgentContext<i64>| async move { ctx.emit(42) });

        let result = handle.result().await.expect("agent should succeed");
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn spawn_agent_with_computation() {
        let handle = spawn(|ctx: AgentContext<i64>| async move {
            let sum = (1..=10).sum();
            ctx.emit(sum)
        });

        let result = handle.result().await.expect("agent should succeed");
        assert_eq!(result, 55);
    }
}
