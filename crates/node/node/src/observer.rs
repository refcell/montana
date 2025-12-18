//! Observer pattern for node events.
//!
//! The [`NodeObserver`] trait allows components to observe node events without
//! tight coupling. Observers are called synchronously and should not block.

use crate::events::NodeEvent;

/// Observer that receives node events.
///
/// Implement this trait to react to node lifecycle and operation events.
/// Implementations should handle events quickly to avoid blocking the node.
pub trait NodeObserver: Send + Sync {
    /// Called when any node event occurs.
    ///
    /// Implementations should handle events quickly to avoid blocking.
    fn on_event(&self, event: &NodeEvent);
}

/// A no-op observer for testing or when no observation is needed.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopObserver;

impl NodeObserver for NoopObserver {
    fn on_event(&self, _event: &NodeEvent) {}
}

/// Adapter that forwards events to an async channel.
///
/// Use this when you need async processing of events. The observer
/// implementation is non-blocking - it sends events to an unbounded channel.
#[derive(Debug)]
pub struct AsyncObserver {
    tx: tokio::sync::mpsc::UnboundedSender<NodeEvent>,
}

impl AsyncObserver {
    /// Create a new async observer and its receiver.
    ///
    /// Events sent to this observer will appear on the returned receiver.
    pub fn new() -> (Self, tokio::sync::mpsc::UnboundedReceiver<NodeEvent>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (Self { tx }, rx)
    }
}

impl Default for AsyncObserver {
    fn default() -> Self {
        Self::new().0
    }
}

impl NodeObserver for AsyncObserver {
    fn on_event(&self, event: &NodeEvent) {
        // Ignore send errors - receiver may have been dropped
        let _ = self.tx.send(event.clone());
    }
}

/// Observer that logs events via tracing.
///
/// Uses appropriate log levels:
/// - `info` for state changes and batch events
/// - `debug` for block events
/// - `error` for error events
#[derive(Debug, Default, Clone, Copy)]
pub struct LoggingObserver;

impl NodeObserver for LoggingObserver {
    fn on_event(&self, event: &NodeEvent) {
        match event {
            NodeEvent::StateChanged(state) => {
                tracing::info!(?state, "node state changed");
            }
            NodeEvent::CheckpointSaved(checkpoint) => {
                tracing::info!(synced_to = checkpoint.synced_to_block, "checkpoint saved");
            }
            NodeEvent::SyncStarted { start_block, target_block } => {
                tracing::info!(start = start_block, target = target_block, "sync started");
            }
            NodeEvent::SyncProgress(progress) => {
                tracing::debug!(
                    current = progress.current_block,
                    target = progress.target_block,
                    "sync progress"
                );
            }
            NodeEvent::SyncCompleted { blocks_synced, duration_secs } => {
                tracing::info!(
                    blocks = blocks_synced,
                    duration_secs = duration_secs,
                    "sync completed"
                );
            }
            NodeEvent::BlockExecuted { block_number, execution_time_ms, gas_used } => {
                tracing::debug!(
                    block = block_number,
                    execution_ms = execution_time_ms,
                    gas_used = gas_used,
                    "block executed"
                );
            }
            NodeEvent::BatchBuilt { batch_number, block_count } => {
                tracing::info!(batch = batch_number, blocks = block_count, "batch built");
            }
            NodeEvent::BatchSubmitted { batch_number, tx_hash } => {
                tracing::info!(
                    batch = batch_number,
                    tx = ?tx_hash,
                    "batch submitted"
                );
            }
            NodeEvent::BatchDerived { batch_number, block_count } => {
                tracing::info!(batch = batch_number, blocks = block_count, "batch derived");
            }
            NodeEvent::BatchValidated { batch_number } => {
                tracing::info!(batch = batch_number, "batch validated");
            }
            NodeEvent::Error { component, message } => {
                tracing::error!(component = component, message = message, "node error");
            }
        }
    }
}
