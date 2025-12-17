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
            // Block events
            NodeEvent::BlockExecuted { block_number } => TuiEvent::UnsafeHeadUpdated(*block_number),

            // Batch events from sequencer
            NodeEvent::BatchSubmitted { batch_number, tx_hash: _ } => {
                // TuiEvent::BatchSubmitted has different fields, use SafeHeadUpdated
                // to indicate batch was submitted
                TuiEvent::SafeHeadUpdated(*batch_number)
            }

            // Batch events from validator
            NodeEvent::BatchDerived { batch_number, block_count } => {
                TuiEvent::BatchDerived {
                    batch_number: *batch_number,
                    block_count: *block_count as u64,
                    first_block: 0, // Not available in NodeEvent
                    last_block: 0,  // Not available in NodeEvent
                }
            }
            NodeEvent::BatchValidated { batch_number } => TuiEvent::FinalizedUpdated(*batch_number),

            // Events we don't forward to TUI
            NodeEvent::StateChanged(_)
            | NodeEvent::CheckpointSaved(_)
            | NodeEvent::SyncStarted { .. }
            | NodeEvent::SyncProgress(_)
            | NodeEvent::SyncCompleted { .. }
            | NodeEvent::BatchBuilt { .. }
            | NodeEvent::Error { .. } => return,
        };

        self.handle.send(tui_event);
    }
}
