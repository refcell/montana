use alloy_primitives::Address;

/// Events that can be sent to the TUI from the montana binary.
///
/// These events represent the key activities happening in the Montana node:
/// - L2 transaction pool activity
/// - L2 block building and batch submission
/// - L1 batch submission with compression
/// - L2 block derivation and execution from L1 data
/// - Chain head progression (unsafe, safe, finalized)
#[derive(Debug, Clone)]
pub enum TuiEvent {
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
}
