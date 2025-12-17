use std::time::Duration;

use alloy_primitives::Address;

/// Events that can be sent to the TUI from the montana binary.
///
/// These events represent the key activities happening in the Montana node:
/// - Sync status and progress updates
/// - L2 block building and batch submission
/// - L1 batch submission with compression
/// - L2 block derivation and execution from L1 data
/// - Chain head progression (unsafe, safe, finalized)
#[derive(Debug, Clone)]
pub enum TuiEvent {
    /// Sync process has started.
    ///
    /// This event is triggered when the node begins syncing from a starting block
    /// to a target block.
    SyncStarted {
        /// The block number at which syncing started
        start_block: u64,
        /// The target block number to reach
        target_block: u64,
    },

    /// Sync progress update.
    ///
    /// This event provides real-time updates on sync progress including
    /// current block, speed, and estimated time remaining.
    SyncProgress {
        /// The current block number being processed
        current_block: u64,
        /// The target block number to reach
        target_block: u64,
        /// Blocks synced per second (rolling average)
        blocks_per_second: f64,
        /// Estimated time remaining
        eta: Option<Duration>,
    },

    /// Sync process has completed.
    ///
    /// This event is triggered when the sync has caught up to the target.
    SyncCompleted {
        /// Total number of blocks synced
        blocks_synced: u64,
        /// Duration of sync in seconds
        duration_secs: f64,
    },

    /// New transaction seen in pool (from L2 fetching).
    ///
    /// This event is triggered when a new transaction arrives in the L2 transaction pool,
    /// typically from the L2 RPC endpoint.
    Transaction {
        /// Transaction hash
        hash: [u8; 32],
        /// Sender address
        from: Address,
        /// Recipient address (None for contract creation)
        to: Option<Address>,
        /// Gas limit for the transaction
        gas_limit: u64,
    },

    /// New L2 block fetched/built.
    ///
    /// This event is triggered when a new L2 block is built or fetched from the L2 endpoint.
    /// It represents the sequencer's block building activity.
    BlockBuilt {
        /// Block number
        number: u64,
        /// Number of transactions in the block
        tx_count: usize,
        /// Total block size in bytes
        size_bytes: usize,
        /// Total gas used in the block
        gas_used: u64,
    },

    /// Batch submitted to L1.
    ///
    /// This event is triggered when a batch of L2 blocks is compressed and submitted
    /// to the L1 data availability layer. It includes compression metrics.
    BatchSubmitted {
        /// Batch number (sequential)
        batch_number: u64,
        /// Number of L2 blocks in this batch
        block_count: usize,
        /// First L2 block number in the batch
        first_block: u64,
        /// Last L2 block number in the batch
        last_block: u64,
        /// Uncompressed batch size in bytes
        uncompressed_size: usize,
        /// Compressed batch size in bytes
        compressed_size: usize,
    },

    /// Block derived and executed from L1.
    ///
    /// This event is triggered when a block is re-derived from L1 batch data,
    /// decompressed, and executed to verify correctness. It includes timing metrics.
    BlockDerived {
        /// Block number
        number: u64,
        /// Time spent deriving the block from L1 data (in milliseconds)
        derivation_time_ms: u64,
        /// Time spent executing the block (in milliseconds)
        execution_time_ms: u64,
    },

    /// Batch derived from L1.
    ///
    /// This event is triggered when a batch is successfully derived from L1 data.
    /// It's used to track round-trip latency (from batch submission to derivation)
    /// and to count the number of batches derived.
    BatchDerived {
        /// Batch number (matches the batch_number from BatchSubmitted)
        batch_number: u64,
        /// Number of blocks in this batch
        block_count: u64,
        /// First block number in the batch
        first_block: u64,
        /// Last block number in the batch
        last_block: u64,
    },

    /// Unsafe head updated (latest L2 block streamed).
    ///
    /// The unsafe head represents the latest L2 block received from the sequencer,
    /// before it has been submitted to L1.
    UnsafeHeadUpdated(u64),

    /// Safe head updated (latest batch submitted block).
    ///
    /// The safe head represents the latest L2 block that has been submitted to L1
    /// in a batch, but may not yet be finalized.
    SafeHeadUpdated(u64),

    /// Finalized head updated (latest re-derived block).
    ///
    /// The finalized head represents the latest L2 block that has been successfully
    /// re-derived and executed from L1 data, providing full consensus guarantees.
    FinalizedUpdated(u64),

    /// Block execution progress update.
    ///
    /// This event is triggered when a block is executed by the sequencer. It allows
    /// the TUI to track execution progress separately from block fetching/building.
    BlockExecuted {
        /// Block number that was executed
        block_number: u64,
        /// Execution time in milliseconds
        execution_time_ms: u64,
    },

    /// Backlog update (blocks fetched vs executed).
    ///
    /// This event tracks the number of blocks fetched from RPC that are waiting
    /// to be executed. Helps visualize the pipeline depth between fetching and execution.
    BacklogUpdated {
        /// Total number of blocks fetched since start
        blocks_fetched: u64,
        /// The last block number that was fetched
        last_fetched_block: u64,
    },

    /// Node mode information.
    ///
    /// This event provides information about the node's operational configuration,
    /// including the node role (Sequencer/Validator/Dual), starting block,
    /// and whether sync was skipped.
    ModeInfo {
        /// Node role (Sequencer, Validator, or Dual)
        node_role: String,
        /// Starting block number (None = from checkpoint)
        start_block: Option<u64>,
        /// Whether sync stage was skipped
        skip_sync: bool,
    },

    /// Pause/Resume toggle.
    ///
    /// This event pauses or resumes the TUI display updates. Useful for inspecting
    /// the current state without new events scrolling the view.
    TogglePause,

    /// Reset the TUI state.
    ///
    /// This event clears all logs, metrics, and statistics, resetting the TUI to
    /// its initial state.
    Reset,

    /// Harness mode enabled.
    ///
    /// This event indicates that Montana is running with the test harness,
    /// which spawns a local anvil chain for testing/demo purposes.
    HarnessModeEnabled,

    /// A new block was produced by the harness anvil instance.
    ///
    /// This event is used to show harness activity in a dedicated TUI section.
    HarnessBlockProduced {
        /// The block number produced by anvil
        block_number: u64,
        /// Number of transactions in the block
        tx_count: usize,
    },
}
