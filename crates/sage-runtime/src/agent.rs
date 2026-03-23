//! Agent spawning and lifecycle management.

use crate::error::{SageError, SageResult};
use crate::llm::LlmClient;
use crate::session::{ProtocolViolation, SenderHandle, SessionId, SharedSessionRegistry};
use std::future::Future;
use tokio::sync::{mpsc, oneshot};

#[cfg(not(target_arch = "wasm32"))]
use tokio::task::JoinHandle;

// ---------------------------------------------------------------------------
// AgentHandle — platform-specific inner field
// ---------------------------------------------------------------------------

/// Handle to a spawned agent.
///
/// This is returned by `spawn()` and can be awaited to get the agent's result.
#[cfg(not(target_arch = "wasm32"))]
pub struct AgentHandle<T> {
    join: JoinHandle<SageResult<T>>,
    message_tx: mpsc::Sender<Message>,
}

/// Handle to a spawned agent (WASM variant).
///
/// Uses a oneshot channel instead of `JoinHandle` since `spawn_local`
/// does not return a handle.
#[cfg(target_arch = "wasm32")]
pub struct AgentHandle<T> {
    result_rx: oneshot::Receiver<SageResult<T>>,
    message_tx: mpsc::Sender<Message>,
}

// ---------------------------------------------------------------------------
// AgentHandle::result() — platform-specific
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
impl<T> AgentHandle<T> {
    /// Wait for the agent to complete and return its result.
    pub async fn result(self) -> SageResult<T> {
        self.join.await?
    }
}

#[cfg(target_arch = "wasm32")]
impl<T> AgentHandle<T> {
    /// Wait for the agent to complete and return its result.
    pub async fn result(self) -> SageResult<T> {
        self.result_rx
            .await
            .map_err(|_| SageError::Agent("Agent task dropped".to_string()))?
    }
}

// ---------------------------------------------------------------------------
// AgentHandle — shared methods (both platforms)
// ---------------------------------------------------------------------------

impl<T> AgentHandle<T> {
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

    /// Send a pre-built message to the agent.
    ///
    /// This is used by generated code when the message needs additional metadata
    /// (like type_name for protocol tracking).
    pub async fn send_message(&self, message: Message) -> SageResult<()> {
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
    /// Phase 3: Session ID for protocol tracking.
    pub session_id: Option<SessionId>,
    /// Phase 3: Handle for replying to this message.
    pub sender: Option<SenderHandle>,
    /// Phase 3: Type name for protocol validation.
    pub type_name: Option<String>,
}

impl Message {
    /// Create a new message from a serializable value.
    pub fn new<T: serde::Serialize>(value: T) -> SageResult<Self> {
        Ok(Self {
            payload: serde_json::to_value(value)?,
            session_id: None,
            sender: None,
            type_name: None,
        })
    }

    /// Create a new message with session context.
    pub fn with_session<T: serde::Serialize>(
        value: T,
        session_id: SessionId,
        sender: SenderHandle,
        type_name: impl Into<String>,
    ) -> SageResult<Self> {
        Ok(Self {
            payload: serde_json::to_value(value)?,
            session_id: Some(session_id),
            sender: Some(sender),
            type_name: Some(type_name.into()),
        })
    }

    /// Set the type name for this message.
    #[must_use]
    pub fn with_type_name(mut self, type_name: impl Into<String>) -> Self {
        self.type_name = Some(type_name.into());
        self
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
    /// Whether emit has been called (prevents double-emit).
    emitted: bool,
    /// Phase 3: The current message being handled (for reply()).
    current_message: Option<Message>,
    /// Phase 3: Session registry for protocol tracking.
    session_registry: SharedSessionRegistry,
    /// Phase 3: The role this agent plays in protocols.
    agent_role: Option<String>,
}

impl<T> AgentContext<T> {
    /// Create a new agent context.
    fn new(
        llm: LlmClient,
        result_tx: oneshot::Sender<T>,
        message_rx: mpsc::Receiver<Message>,
        session_registry: SharedSessionRegistry,
    ) -> Self {
        Self {
            llm,
            result_tx: Some(result_tx),
            message_rx,
            emitted: false,
            current_message: None,
            session_registry,
            agent_role: None,
        }
    }

    /// Set the role this agent plays in protocols.
    pub fn set_role(&mut self, role: impl Into<String>) {
        self.agent_role = Some(role.into());
    }

    /// Get the session registry.
    #[must_use]
    pub fn session_registry(&self) -> &SharedSessionRegistry {
        &self.session_registry
    }

    /// Emit a value to the awaiter.
    ///
    /// This should be called once at the end of the agent's execution.
    /// Calling emit multiple times is a no-op after the first call.
    pub fn emit(&mut self, value: T) -> SageResult<T>
    where
        T: Clone,
    {
        if self.emitted {
            // Already emitted, just return the value
            return Ok(value);
        }
        self.emitted = true;
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

        // Phase 3: Store current message for reply()
        self.current_message = Some(msg.clone());

        serde_json::from_value(msg.payload)
            .map_err(|e| SageError::Agent(format!("Failed to deserialize message: {e}")))
    }

    /// Receive a message with a timeout.
    ///
    /// Returns `None` if the timeout expires before a message arrives.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn receive_timeout<M>(
        &mut self,
        timeout: std::time::Duration,
    ) -> SageResult<Option<M>>
    where
        M: serde::de::DeserializeOwned,
    {
        match tokio::time::timeout(timeout, self.message_rx.recv()).await {
            Ok(Some(msg)) => {
                // Phase 3: Store current message for reply()
                self.current_message = Some(msg.clone());

                let value = serde_json::from_value(msg.payload)
                    .map_err(|e| SageError::Agent(format!("Failed to deserialize message: {e}")))?;
                Ok(Some(value))
            }
            Ok(None) => Err(SageError::Agent("Message channel closed".to_string())),
            Err(_) => Ok(None), // Timeout
        }
    }

    /// Receive a message with a timeout (WASM variant).
    ///
    /// Uses browser `setTimeout` for the timeout mechanism.
    #[cfg(target_arch = "wasm32")]
    pub async fn receive_timeout<M>(
        &mut self,
        timeout: std::time::Duration,
    ) -> SageResult<Option<M>>
    where
        M: serde::de::DeserializeOwned,
    {
        use futures::future::{select, Either};
        use std::pin::pin;

        let recv_fut = pin!(self.message_rx.recv());
        let sleep_fut = pin!(sage_runtime_web::sleep(timeout));

        match select(recv_fut, sleep_fut).await {
            Either::Left((Some(msg), _)) => {
                self.current_message = Some(msg.clone());
                let value = serde_json::from_value(msg.payload)
                    .map_err(|e| SageError::Agent(format!("Failed to deserialize message: {e}")))?;
                Ok(Some(value))
            }
            Either::Left((None, _)) => {
                Err(SageError::Agent("Message channel closed".to_string()))
            }
            Either::Right((_, _)) => Ok(None), // Timeout
        }
    }

    /// Receive the raw message from the agent's mailbox.
    ///
    /// This blocks until a message is available. Returns the full Message
    /// including session context.
    pub async fn receive_raw(&mut self) -> SageResult<Message> {
        let msg = self
            .message_rx
            .recv()
            .await
            .ok_or_else(|| SageError::Agent("Message channel closed".to_string()))?;

        // Store current message for reply()
        self.current_message = Some(msg.clone());

        Ok(msg)
    }

    /// Set the current message context (for use in message handlers).
    ///
    /// This is called by generated code when entering a message handler.
    pub fn set_current_message(&mut self, msg: Message) {
        self.current_message = Some(msg);
    }

    /// Clear the current message context (for use after message handlers).
    pub fn clear_current_message(&mut self) {
        self.current_message = None;
    }

    /// Phase 3: Reply to the current message.
    ///
    /// This sends a response back to the sender of the current message.
    /// Can only be called inside a message handler.
    ///
    /// # Errors
    ///
    /// Returns an error if called outside a message handler or if
    /// the current message has no sender handle.
    pub async fn reply<M: serde::Serialize>(&mut self, msg: M) -> SageResult<()> {
        let current = self
            .current_message
            .as_ref()
            .ok_or_else(|| SageError::from(ProtocolViolation::ReplyOutsideHandler))?;

        let sender = current
            .sender
            .as_ref()
            .ok_or_else(|| SageError::Agent("Message has no sender handle".to_string()))?;

        sender.send(msg).await
    }

    /// Phase 3: Reply to the current message with protocol state validation.
    pub async fn reply_with_protocol<M: serde::Serialize>(
        &mut self,
        msg: M,
        msg_type: &str,
        role: &str,
    ) -> SageResult<()> {
        let current = self
            .current_message
            .as_ref()
            .ok_or_else(|| SageError::from(ProtocolViolation::ReplyOutsideHandler))?;

        // If message has a session, validate protocol state
        if let Some(session_id) = current.session_id {
            let mut registry = self.session_registry.write().await;
            if let Some(session) = registry.get_mut(&session_id) {
                // Validate that we can send this message type from our role
                if !session.state.can_send(msg_type, role) {
                    return Err(SageError::from(ProtocolViolation::UnexpectedMessage {
                        protocol: session.protocol.clone(),
                        expected: "valid reply".to_string(),
                        received: msg_type.to_string(),
                        state: session.state.state_name().to_string(),
                    }));
                }
                // Transition the state machine
                session.state.transition(msg_type)?;
            }
        }

        let sender = current
            .sender
            .as_ref()
            .ok_or_else(|| SageError::Agent("Message has no sender handle".to_string()))?;

        sender.send(msg).await
    }

    /// Phase 3: Validate incoming message against protocol state.
    pub async fn validate_protocol_receive(
        &mut self,
        msg_type: &str,
        role: &str,
    ) -> SageResult<()> {
        let current = match &self.current_message {
            Some(msg) => msg,
            None => return Ok(()), // No current message, nothing to validate
        };

        // If message has a session, validate protocol state
        if let Some(session_id) = current.session_id {
            let mut registry = self.session_registry.write().await;
            if let Some(session) = registry.get_mut(&session_id) {
                // Validate that we can receive this message type in our role
                if !session.state.can_receive(msg_type, role) {
                    return Err(SageError::from(ProtocolViolation::UnexpectedMessage {
                        protocol: session.protocol.clone(),
                        expected: "valid message for current state".to_string(),
                        received: msg_type.to_string(),
                        state: session.state.state_name().to_string(),
                    }));
                }
                // Transition the state machine
                session.state.transition(msg_type)?;

                // If protocol is complete, remove the session
                if session.state.is_terminal() {
                    drop(registry);
                    self.session_registry.write().await.remove(&session_id);
                }
            }
        }

        Ok(())
    }

    /// Phase 3: Start a new protocol session.
    pub async fn start_session(
        &self,
        protocol: String,
        role: String,
        state: Box<dyn crate::session::ProtocolStateMachine>,
        partner: SenderHandle,
    ) -> SessionId {
        let mut registry = self.session_registry.write().await;
        let session_id = registry.next_id();
        registry.start_session(session_id, protocol, role, state, partner);
        session_id
    }

    /// Get the current message being handled (if any).
    #[must_use]
    pub fn current_message(&self) -> Option<&Message> {
        self.current_message.as_ref()
    }
}

// ---------------------------------------------------------------------------
// spawn — native (tokio::spawn, requires Send)
// ---------------------------------------------------------------------------

/// Spawn an agent and return a handle to it.
///
/// The agent will run asynchronously in a separate task.
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn<A, T, F>(agent: A) -> AgentHandle<T>
where
    A: FnOnce(AgentContext<T>) -> F + Send + 'static,
    F: Future<Output = SageResult<T>> + Send,
    T: Send + 'static,
{
    spawn_with_llm_config(agent, crate::llm::LlmConfig::from_env())
}

/// Spawn an agent with a custom LLM configuration.
///
/// This is used by effect handlers to configure per-agent LLM settings.
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_with_llm_config<A, T, F>(agent: A, llm_config: crate::llm::LlmConfig) -> AgentHandle<T>
where
    A: FnOnce(AgentContext<T>) -> F + Send + 'static,
    F: Future<Output = SageResult<T>> + Send,
    T: Send + 'static,
{
    let (result_tx, result_rx) = oneshot::channel();
    let (message_tx, message_rx) = mpsc::channel(32);

    let llm = LlmClient::new(llm_config);
    let session_registry = crate::session::shared_registry();
    let ctx = AgentContext::new(llm, result_tx, message_rx, session_registry);

    let join = tokio::spawn(async move { agent(ctx).await });

    // We need to handle the result_rx somewhere, but for now we just let
    // the result come from the JoinHandle
    drop(result_rx);

    AgentHandle { join, message_tx }
}

// ---------------------------------------------------------------------------
// spawn — WASM (spawn_local, no Send bounds)
// ---------------------------------------------------------------------------

/// Spawn an agent and return a handle to it.
///
/// On WASM, agents run on the browser's single-threaded event loop
/// via `spawn_local`. No `Send` bounds are required.
#[cfg(target_arch = "wasm32")]
pub fn spawn<A, T, F>(agent: A) -> AgentHandle<T>
where
    A: FnOnce(AgentContext<T>) -> F + 'static,
    F: Future<Output = SageResult<T>> + 'static,
    T: 'static,
{
    spawn_with_llm_config(agent, crate::llm::LlmConfig::from_env())
}

/// Spawn an agent with a custom LLM configuration (WASM variant).
#[cfg(target_arch = "wasm32")]
pub fn spawn_with_llm_config<A, T, F>(agent: A, llm_config: crate::llm::LlmConfig) -> AgentHandle<T>
where
    A: FnOnce(AgentContext<T>) -> F + 'static,
    F: Future<Output = SageResult<T>> + 'static,
    T: 'static,
{
    let (task_result_tx, task_result_rx) = oneshot::channel();
    let (emit_tx, _emit_rx) = oneshot::channel();
    let (message_tx, message_rx) = mpsc::channel(32);

    let llm = LlmClient::new(llm_config);
    let session_registry = crate::session::shared_registry();
    let ctx = AgentContext::new(llm, emit_tx, message_rx, session_registry);

    wasm_bindgen_futures::spawn_local(async move {
        let result = agent(ctx).await;
        let _ = task_result_tx.send(result);
    });

    AgentHandle {
        result_rx: task_result_rx,
        message_tx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[tokio::test]
    async fn spawn_simple_agent() {
        let handle = spawn(|mut ctx: AgentContext<i64>| async move { ctx.emit(42) });

        let result = handle.result().await.expect("agent should succeed");
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn spawn_agent_with_computation() {
        let handle = spawn(|mut ctx: AgentContext<i64>| async move {
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
