//! TUI observer for node events.
//!
//! The [`TuiObserver`] converts [`NodeEvent`]s to [`TuiEvent`]s and forwards
//! them to the TUI via a [`TuiHandle`].

use montana_node::{NodeEvent, NodeObserver};

use crate::{TuiEvent, TuiHandle};

/// Adapter that converts NodeEvents to TuiEvents and sends to TUI.
///
/// This observer enables the TUI to receive node events without the node
/// having direct knowledge of the TUI.
#[derive(Debug, Clone)]
pub struct TuiObserver {
    handle: TuiHandle,
}

impl TuiObserver {
    /// Create a new TUI observer with the given handle.
    pub const fn new(handle: TuiHandle) -> Self {
        Self { handle }
    }
}

impl NodeObserver for TuiObserver {
    fn on_event(&self, event: &NodeEvent) {
        // Convert NodeEvent to TuiEvent where applicable
        let tui_event = match event {
            // Sync events
            NodeEvent::SyncStarted { start_block, target_block } => {
                TuiEvent::SyncStarted { start_block: *start_block, target_block: *target_block }
            }
            NodeEvent::SyncProgress(progress) => TuiEvent::SyncProgress {
                current_block: progress.current_block,
                target_block: progress.target_block,
                blocks_per_second: progress.blocks_per_second,
                eta: progress.eta,
            },
            NodeEvent::SyncCompleted { blocks_synced, duration_secs } => TuiEvent::SyncCompleted {
                blocks_synced: *blocks_synced,
                duration_secs: *duration_secs,
            },

            // Block events
            NodeEvent::BlockExecuted { block_number, execution_time_ms } => {
                TuiEvent::BlockExecuted {
                    block_number: *block_number,
                    execution_time_ms: *execution_time_ms,
                }
            }

            // Batch events from sequencer
            NodeEvent::BatchSubmitted { batch_number, tx_hash: _ } => {
                // TuiEvent::BatchSubmitted has different fields, use SafeHeadUpdated
                // to indicate batch was submitted
                TuiEvent::SafeHeadUpdated(*batch_number)
            }

            // Batch events from validator
            NodeEvent::BatchDerived { batch_number, block_count } => TuiEvent::BatchDerived {
                batch_number: *batch_number,
                block_count: *block_count as u64,
                first_block: 0, // Not available in NodeEvent
                last_block: 0,  // Not available in NodeEvent
            },
            NodeEvent::BatchValidated { batch_number } => TuiEvent::FinalizedUpdated(*batch_number),

            // Events we don't forward to TUI
            NodeEvent::StateChanged(_)
            | NodeEvent::CheckpointSaved(_)
            | NodeEvent::BatchBuilt { .. }
            | NodeEvent::Error { .. } => return,
        };

        self.handle.send(tui_event);
    }
}
