//! Supervision tree implementation for Sage v2.
//!
//! This module provides Erlang/OTP-style supervision trees for managing
//! agent lifecycles with automatic restart capabilities.
//!
//! # Supervision Strategies
//!
//! - **OneForOne**: Restart only the failed child
//! - **OneForAll**: Restart all children if one fails
//! - **RestForOne**: Restart the failed child and all children started after it
//!
//! # Restart Policies
//!
//! - **Permanent**: Always restart, regardless of exit reason
//! - **Transient**: Restart only on abnormal termination (error)
//! - **Temporary**: Never restart
//!
//! # Example
//!
//! ```ignore
//! use sage_runtime::supervisor::{Supervisor, Strategy, RestartPolicy};
//!
//! let mut supervisor = Supervisor::new(Strategy::OneForOne, Default::default());
//!
//! supervisor.add_child("Worker", RestartPolicy::Permanent, || {
//!     sage_runtime::spawn(|mut ctx| async move {
//!         // Agent logic
//!         ctx.emit(())
//!     })
//! });
//!
//! supervisor.run().await?;
//! ```

use crate::error::{SageError, SageResult};
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;

/// Supervision strategy (OTP-inspired).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Strategy {
    /// Restart only the failed child.
    #[default]
    OneForOne,
    /// Restart all children if one fails.
    OneForAll,
    /// Restart the failed child and all children started after it.
    RestForOne,
}

/// Restart policy for supervised children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RestartPolicy {
    /// Always restart, regardless of exit reason.
    #[default]
    Permanent,
    /// Restart only on abnormal termination (error).
    Transient,
    /// Never restart.
    Temporary,
}

/// Configuration for restart intensity limiting (circuit breaker).
#[derive(Debug, Clone)]
pub struct RestartConfig {
    /// Maximum number of restarts allowed within the time window.
    pub max_restarts: u32,
    /// Time window in which max_restarts is measured.
    pub within: Duration,
}

impl Default for RestartConfig {
    fn default() -> Self {
        Self {
            max_restarts: 5,
            within: Duration::from_secs(60),
        }
    }
}

/// Tracks restart history for circuit breaker functionality.
struct RestartTracker {
    timestamps: VecDeque<Instant>,
    config: RestartConfig,
}

impl RestartTracker {
    fn new(config: RestartConfig) -> Self {
        Self {
            timestamps: VecDeque::new(),
            config,
        }
    }

    /// Record a restart and check if we've exceeded the limit.
    /// Returns true if we should allow the restart, false if circuit breaker trips.
    fn record_restart(&mut self) -> bool {
        let now = Instant::now();

        // Remove old timestamps outside the window
        while let Some(&oldest) = self.timestamps.front() {
            if now.duration_since(oldest) > self.config.within {
                self.timestamps.pop_front();
            } else {
                break;
            }
        }

        // Check if we're at the limit
        if self.timestamps.len() >= self.config.max_restarts as usize {
            return false; // Circuit breaker trips
        }

        self.timestamps.push_back(now);
        true
    }
}

/// A spawn function that creates an agent and returns its join handle.
pub type SpawnFn = Box<dyn Fn() -> Pin<Box<dyn Future<Output = SageResult<()>> + Send>> + Send>;

/// Handle to a supervised child.
struct ChildHandle {
    name: String,
    restart_policy: RestartPolicy,
    spawn_fn: SpawnFn,
    handle: Option<JoinHandle<SageResult<()>>>,
}

impl ChildHandle {
    fn new(name: String, restart_policy: RestartPolicy, spawn_fn: SpawnFn) -> Self {
        Self {
            name,
            restart_policy,
            spawn_fn,
            handle: None,
        }
    }

    /// Spawn (or respawn) this child.
    fn spawn(&mut self) {
        let future = (self.spawn_fn)();
        self.handle = Some(tokio::spawn(async move { future.await }));
    }

    /// Check if the child is running.
    fn is_running(&self) -> bool {
        self.handle
            .as_ref()
            .map(|h| !h.is_finished())
            .unwrap_or(false)
    }

    /// Take the join handle (for awaiting).
    fn take_handle(&mut self) -> Option<JoinHandle<SageResult<()>>> {
        self.handle.take()
    }
}

/// A supervisor that manages child agents with restart strategies.
pub struct Supervisor {
    strategy: Strategy,
    children: Vec<ChildHandle>,
    restart_tracker: RestartTracker,
}

impl Supervisor {
    /// Create a new supervisor with the given strategy and restart configuration.
    pub fn new(strategy: Strategy, config: RestartConfig) -> Self {
        Self {
            strategy,
            children: Vec::new(),
            restart_tracker: RestartTracker::new(config),
        }
    }

    /// Add a child to the supervisor.
    ///
    /// The spawn function should create the agent and return its future.
    pub fn add_child<F, Fut>(&mut self, name: impl Into<String>, restart_policy: RestartPolicy, spawn_fn: F)
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: Future<Output = SageResult<()>> + Send + 'static,
    {
        let spawn_fn: SpawnFn = Box::new(move || Box::pin(spawn_fn()));
        self.children.push(ChildHandle::new(name.into(), restart_policy, spawn_fn));
    }

    /// Start all children and begin supervision.
    ///
    /// This method runs until all children have terminated (according to their
    /// restart policies) or the circuit breaker trips.
    pub async fn run(&mut self) -> SageResult<()> {
        // Start all children
        for child in &mut self.children {
            child.spawn();
        }

        // Monitor loop
        loop {
            // Wait for any child to complete
            let (index, result) = self.wait_for_child_exit().await;

            // Check if all children are done
            if index.is_none() {
                // All children have finished
                break;
            }

            let index = index.unwrap();
            let child_name = self.children[index].name.clone();
            let restart_policy = self.children[index].restart_policy;

            // Determine if we should restart
            let should_restart = match (restart_policy, &result) {
                (RestartPolicy::Permanent, _) => true,
                (RestartPolicy::Transient, Err(_)) => true,
                (RestartPolicy::Transient, Ok(_)) => false,
                (RestartPolicy::Temporary, _) => false,
            };

            if should_restart {
                // Check circuit breaker
                if !self.restart_tracker.record_restart() {
                    return Err(SageError::Supervisor(format!(
                        "Maximum restart intensity reached for supervisor (child '{}' failed too many times)",
                        child_name
                    )));
                }

                // Apply restart strategy
                match self.strategy {
                    Strategy::OneForOne => {
                        self.restart_child(index);
                    }
                    Strategy::OneForAll => {
                        self.restart_all();
                    }
                    Strategy::RestForOne => {
                        self.restart_rest(index);
                    }
                }
            }

            // Check if any children are still running
            if !self.any_running() {
                break;
            }
        }

        Ok(())
    }

    /// Wait for any child to exit, returning the index and result.
    async fn wait_for_child_exit(&mut self) -> (Option<usize>, SageResult<()>) {
        use futures::future::select_all;

        // Collect all running children's handles with their indices
        let handles_with_indices: Vec<(usize, JoinHandle<SageResult<()>>)> = self
            .children
            .iter_mut()
            .enumerate()
            .filter_map(|(i, c)| c.take_handle().map(|h| (i, h)))
            .collect();

        if handles_with_indices.is_empty() {
            return (None, Ok(()));
        }

        // We need to track indices separately since select_all works on the handles
        let indices: Vec<usize> = handles_with_indices.iter().map(|(i, _)| *i).collect();
        let handles: Vec<JoinHandle<SageResult<()>>> =
            handles_with_indices.into_iter().map(|(_, h)| h).collect();

        // Wait for any handle to complete
        let (join_result, completed_idx, remaining_handles) = select_all(handles).await;

        // Get the original child index
        let child_index = indices[completed_idx];

        // Convert JoinError to SageError
        let final_result = join_result.unwrap_or_else(|e| Err(SageError::Agent(e.to_string())));

        // Put back the remaining handles to their respective children
        // Build list of (handle, original_index) pairs for non-completed handles
        let mut remaining_iter = remaining_handles.into_iter();
        for (pos, &original_idx) in indices.iter().enumerate() {
            if pos != completed_idx {
                if let (Some(handle), Some(child)) =
                    (remaining_iter.next(), self.children.get_mut(original_idx))
                {
                    child.handle = Some(handle);
                }
            }
        }

        (Some(child_index), final_result)
    }

    /// Restart a single child.
    fn restart_child(&mut self, index: usize) {
        if let Some(child) = self.children.get_mut(index) {
            child.spawn();
        }
    }

    /// Restart all children (stop all first, then start all).
    fn restart_all(&mut self) {
        // Abort all running children
        for child in &mut self.children {
            if let Some(handle) = child.take_handle() {
                handle.abort();
            }
        }

        // Start all children
        for child in &mut self.children {
            child.spawn();
        }
    }

    /// Restart the failed child and all children started after it.
    fn restart_rest(&mut self, from_index: usize) {
        // Abort children from index onwards
        for child in self.children.iter_mut().skip(from_index) {
            if let Some(handle) = child.take_handle() {
                handle.abort();
            }
        }

        // Restart children from index onwards
        for child in self.children.iter_mut().skip(from_index) {
            child.spawn();
        }
    }

    /// Check if any children are still running.
    fn any_running(&self) -> bool {
        self.children.iter().any(|c| c.is_running())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_one_for_one_restart() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let mut supervisor = Supervisor::new(Strategy::OneForOne, RestartConfig::default());

        // Use Transient policy - restart on error, stop on success
        supervisor.add_child("Worker", RestartPolicy::Transient, move || {
            let counter = counter_clone.clone();
            async move {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(SageError::Agent("Simulated failure".to_string()))
                } else {
                    Ok(())
                }
            }
        });

        let result = supervisor.run().await;
        assert!(result.is_ok(), "supervisor failed: {:?}", result);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_transient_no_restart_on_success() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let mut supervisor = Supervisor::new(Strategy::OneForOne, RestartConfig::default());

        supervisor.add_child("Worker", RestartPolicy::Transient, move || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });

        let result = supervisor.run().await;
        assert!(result.is_ok());
        assert_eq!(counter.load(Ordering::SeqCst), 1); // Only ran once
    }

    #[tokio::test]
    async fn test_temporary_never_restarts() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let mut supervisor = Supervisor::new(Strategy::OneForOne, RestartConfig::default());

        supervisor.add_child("Worker", RestartPolicy::Temporary, move || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(SageError::Agent("Simulated failure".to_string()))
            }
        });

        let result = supervisor.run().await;
        assert!(result.is_ok()); // Supervisor should succeed even if child fails
        assert_eq!(counter.load(Ordering::SeqCst), 1); // Only ran once
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = RestartConfig {
            max_restarts: 3,
            within: Duration::from_secs(60),
        };

        let mut supervisor = Supervisor::new(Strategy::OneForOne, config);

        supervisor.add_child("Worker", RestartPolicy::Permanent, move || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(SageError::Agent("Always fails".to_string()))
            }
        });

        let result = supervisor.run().await;
        assert!(result.is_err()); // Circuit breaker should trip
        assert!(counter.load(Ordering::SeqCst) <= 4); // At most 4 attempts (1 + 3 restarts)
    }

    #[tokio::test]
    async fn test_permanent_restarts_on_success() {
        // Permanent policy restarts even when child exits normally.
        // This test verifies the circuit breaker eventually stops it.
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let config = RestartConfig {
            max_restarts: 3,
            within: Duration::from_secs(60),
        };

        let mut supervisor = Supervisor::new(Strategy::OneForOne, config);

        supervisor.add_child("Worker", RestartPolicy::Permanent, move || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(()) // Exits successfully each time
            }
        });

        let result = supervisor.run().await;
        // Circuit breaker trips because Permanent keeps restarting even on success
        assert!(result.is_err());
        assert!(counter.load(Ordering::SeqCst) <= 4);
    }
}
