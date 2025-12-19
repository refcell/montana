use montana_checkpoint::Checkpoint;

use crate::{SyncProgress, state::NodeState};

/// Events emitted by the node for observers
#[derive(Debug, Clone)]
pub enum NodeEvent {
    // Lifecycle events
    /// Node state has changed
    StateChanged(NodeState),
    /// A checkpoint has been saved to disk
    CheckpointSaved(Checkpoint),

    // Sync events
    /// Sync process has started
    SyncStarted {
        /// The block number at which syncing started
        start_block: u64,
        /// The target block number to reach
        target_block: u64,
    },
    /// Sync progress update
    SyncProgress(SyncProgress),
    /// Sync process has completed
    SyncCompleted {
        /// Total number of blocks synced
        blocks_synced: u64,
        /// Duration of sync in seconds
        duration_secs: f64,
    },

    // Sequencer events
    /// A block has been executed
    BlockExecuted {
        /// The block number that was executed
        block_number: u64,
        /// Execution time in microseconds (for sub-millisecond precision)
        execution_time_us: u64,
        /// Gas used by the block
        gas_used: u64,
    },
    /// A batch has been built
    BatchBuilt {
        /// The batch number that was built
        batch_number: u64,
        /// Number of blocks in the batch
        block_count: usize,
    },
    /// A batch has been submitted to L1
    BatchSubmitted {
        /// The batch number that was submitted
        batch_number: u64,
        /// Transaction hash of the submission
        tx_hash: [u8; 32],
    },

    // Validator events
    /// A batch has been derived from L1
    BatchDerived {
        /// The batch number that was derived
        batch_number: u64,
        /// Number of blocks in the batch
        block_count: usize,
    },
    /// A batch has been validated
    BatchValidated {
        /// The batch number that was validated
        batch_number: u64,
    },

    // Error events
    /// An error has occurred
    Error {
        /// Component where the error occurred
        component: String,
        /// Error message
        message: String,
    },
}
