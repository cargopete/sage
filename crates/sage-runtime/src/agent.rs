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
    message_tx: mpsc::Sender<Message>,
}

impl<T> AgentHandle<T> {
    /// Wait for the agent to complete and return its result.
    pub async fn result(self) -> SageResult<T> {
        self.join.await?
    }

    /// Send a message to the agent.
    ///
    /// The message will be serialized to JSON and placed in the agent's mailbox.
    pub async fn send<M>(&self, msg: M) -> SageResult<()>
    where
        M: serde::Serialize,
    {
        let message = Message::new(msg)?;
        self.message_tx
            .send(message)
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
    /// Channel to receive messages from other agents.
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

    /// Receive a message from the agent's mailbox.
    ///
    /// This blocks until a message is available. The message is deserialized
    /// into the specified type.
    pub async fn receive<M>(&mut self) -> SageResult<M>
    where
        M: serde::de::DeserializeOwned,
    {
        let msg = self
            .message_rx
            .recv()
            .await
            .ok_or_else(|| SageError::Agent("Message channel closed".to_string()))?;

        serde_json::from_value(msg.payload)
            .map_err(|e| SageError::Agent(format!("Failed to deserialize message: {e}")))
    }

    /// Receive a message with a timeout.
    ///
    /// Returns `None` if the timeout expires before a message arrives.
    pub async fn receive_timeout<M>(
        &mut self,
        timeout: std::time::Duration,
    ) -> SageResult<Option<M>>
    where
        M: serde::de::DeserializeOwned,
    {
        match tokio::time::timeout(timeout, self.message_rx.recv()).await {
            Ok(Some(msg)) => {
                let value = serde_json::from_value(msg.payload)
                    .map_err(|e| SageError::Agent(format!("Failed to deserialize message: {e}")))?;
                Ok(Some(value))
            }
            Ok(None) => Err(SageError::Agent("Message channel closed".to_string())),
            Err(_) => Ok(None), // Timeout
        }
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
    use serde::{Deserialize, Serialize};

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

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TaskMessage {
        id: u32,
        content: String,
    }

    #[tokio::test]
    async fn agent_receives_message() {
        let handle = spawn(|mut ctx: AgentContext<String>| async move {
            let msg: TaskMessage = ctx.receive().await?;
            ctx.emit(format!("Got task {}: {}", msg.id, msg.content))
        });

        handle
            .send(TaskMessage {
                id: 42,
                content: "Hello".to_string(),
            })
            .await
            .expect("send should succeed");

        let result = handle.result().await.expect("agent should succeed");
        assert_eq!(result, "Got task 42: Hello");
    }

    #[tokio::test]
    async fn agent_receives_multiple_messages() {
        let handle = spawn(|mut ctx: AgentContext<i32>| async move {
            let mut sum = 0;
            for _ in 0..3 {
                let n: i32 = ctx.receive().await?;
                sum += n;
            }
            ctx.emit(sum)
        });

        for n in [10, 20, 30] {
            handle.send(n).await.expect("send should succeed");
        }

        let result = handle.result().await.expect("agent should succeed");
        assert_eq!(result, 60);
    }

    #[tokio::test]
    async fn agent_receive_timeout() {
        let handle = spawn(|mut ctx: AgentContext<String>| async move {
            let result: Option<i32> = ctx
                .receive_timeout(std::time::Duration::from_millis(10))
                .await?;
            match result {
                Some(n) => ctx.emit(format!("Got {n}")),
                None => ctx.emit("Timeout".to_string()),
            }
        });

        // Don't send anything, let it timeout
        let result = handle.result().await.expect("agent should succeed");
        assert_eq!(result, "Timeout");
    }
}
