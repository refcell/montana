use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use montana_tui_common::LogEntry;

/// Maximum number of log entries to keep in each log buffer.
const MAX_LOG_ENTRIES: usize = 100;

/// Maximum number of L1 blocks to keep for visualization.
const MAX_L1_BLOCKS: usize = 50;

/// Represents an L1 block for visualization purposes.
#[derive(Debug, Clone)]
pub struct L1Block {
    /// The L1 block number
    pub number: u64,
    /// Optional batch number if this block contains a batch submission
    pub batch_number: Option<u64>,
    /// Number of L2 blocks in the batch (if any)
    pub batch_block_count: Option<usize>,
    /// Whether this block is the origin of a derived batch
    pub is_derivation_origin: bool,
}

/// Sync state tracking
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyncState {
    /// Not syncing / idle
    Idle,
    /// Actively syncing
    Syncing {
        /// Starting block
        start_block: u64,
        /// Current block
        current_block: u64,
        /// Target block
        target_block: u64,
        /// Blocks per second
        blocks_per_second: f64,
        /// ETA
        eta: Option<Duration>,
    },
    /// Sync completed
    Completed {
        /// Blocks synced
        blocks_synced: u64,
        /// Duration in seconds
        duration_secs: f64,
    },
}

/// Application state for the Montana TUI.
///
/// This struct maintains all the state needed to render the 4-pane TUI:
/// - Chain head progression (unsafe, safe, finalized)
/// - Sync state and progress metrics
/// - Statistics (blocks processed, batches submitted/derived)
/// - Round-trip latency metrics
/// - Log buffers for each component
/// - Pause state
///
/// The state is updated by processing [`TuiEvent`](crate::TuiEvent)s received
/// from the Montana binary.
#[derive(Debug)]
pub struct App {
    // Chain state
    /// Latest unsafe head (L2 block streamed from sequencer)
    pub unsafe_head: u64,
    /// Latest safe head (L2 block submitted to L1)
    pub safe_head: u64,
    /// Latest finalized head (L2 block re-derived from L1)
    pub finalized_head: u64,

    // Sync state
    /// Current sync state
    pub sync_state: SyncState,
    /// Total syncs completed
    pub syncs_completed: u64,
    /// Timestamp when sync started (for elapsed time tracking)
    pub sync_start_time: Option<Instant>,

    // Statistics
    /// Total number of L2 blocks processed
    pub blocks_processed: u64,
    /// Total number of batches submitted to L1
    pub batches_submitted: u64,
    /// Total number of batches derived from L1
    pub batches_derived: u64,

    // Transaction tracking
    /// Total transactions received by the sequencer
    pub transactions_received: u64,
    /// Timestamp when first transaction was received (for rate calculation)
    pub tx_tracking_start_time: Option<Instant>,

    // Execution tracking
    /// Total blocks fetched (for backlog calculation)
    pub blocks_fetched: u64,
    /// Total blocks executed
    pub blocks_executed: u64,
    /// Last fetched block number
    pub last_fetched_block: u64,
    /// Last executed block number
    pub last_executed_block: u64,
    /// Recent execution times in ms (for calculating rate)
    execution_times: Vec<u64>,
    /// Total gas used across all executed blocks
    total_gas_used: u128,
    /// Gas used in recent blocks (for calculating gigagas/s rate)
    recent_gas_used: Vec<u64>,
    /// Timestamp when execution started (for rate calculation)
    pub execution_start_time: Option<std::time::Instant>,
    /// Timestamp when last block was executed (for stall detection)
    pub last_execution_time: Option<std::time::Instant>,
    /// Execution logs
    pub execution_logs: Vec<LogEntry>,

    // Round-trip latency tracking
    /// Batch submission timestamps (batch_number -> submit time)
    batch_submit_times: HashMap<u64, Instant>,
    /// Round-trip latencies in milliseconds (submit to re-derive)
    round_trip_latencies: Vec<u64>,

    // Compression tracking
    /// Compression ratios as percentages (compressed/uncompressed * 100)
    compression_ratios: Vec<f64>,

    // Log buffers
    /// Sync update logs (replaces tx_pool_logs)
    pub sync_logs: Vec<LogEntry>,
    /// Block builder logs
    pub block_builder_logs: Vec<LogEntry>,
    /// Batch submission logs
    pub batch_logs: Vec<LogEntry>,
    /// Derivation logs
    pub derivation_logs: Vec<LogEntry>,
    /// Derivation execution logs (block execution during derivation)
    pub derivation_execution_logs: Vec<LogEntry>,

    // Mode information
    /// Node role (Sequencer, Validator, or Dual)
    pub node_role: String,
    /// Starting block number (None = from checkpoint)
    pub start_block: Option<u64>,
    /// Whether sync stage was skipped
    pub skip_sync: bool,

    // UI state
    /// Whether the TUI is paused (no updates)
    pub is_paused: bool,
    /// Whether harness mode is enabled (testing/demo mode)
    pub harness_mode: bool,

    // Harness tracking
    /// Harness block logs (only populated in harness mode)
    pub harness_logs: Vec<LogEntry>,
    /// Latest harness block number
    pub harness_block: u64,
    /// Block number where harness initialization completed (blocks <= this were logged during init)
    pub harness_init_block: u64,

    // L1 chain visualization
    /// L1 blocks for visualization (scrolling block display)
    pub l1_blocks: Vec<L1Block>,
    /// Current L1 block number
    pub l1_head: u64,
    /// L1 block number containing the latest batch submission (for cursor display)
    pub latest_batch_submission_l1_block: Option<u64>,
    /// L1 block number that is the origin of the latest derived batch (for cursor display)
    pub latest_derivation_origin_l1_block: Option<u64>,
    /// Mapping from batch number to the L1 block where it was submitted
    batch_to_l1_block: HashMap<u64, u64>,
}

impl App {
    /// Create a new application state.
    ///
    /// All fields are initialized to their default values (zeros, empty collections).
    pub fn new() -> Self {
        Self {
            unsafe_head: 0,
            safe_head: 0,
            finalized_head: 0,
            sync_state: SyncState::Idle,
            syncs_completed: 0,
            sync_start_time: None,
            blocks_processed: 0,
            batches_submitted: 0,
            batches_derived: 0,
            transactions_received: 0,
            tx_tracking_start_time: None,
            blocks_fetched: 0,
            blocks_executed: 0,
            last_fetched_block: 0,
            last_executed_block: 0,
            execution_times: Vec::new(),
            total_gas_used: 0,
            recent_gas_used: Vec::new(),
            execution_start_time: None,
            last_execution_time: None,
            execution_logs: Vec::new(),
            batch_submit_times: HashMap::new(),
            round_trip_latencies: Vec::new(),
            compression_ratios: Vec::new(),
            sync_logs: Vec::new(),
            block_builder_logs: Vec::new(),
            batch_logs: Vec::new(),
            derivation_logs: Vec::new(),
            derivation_execution_logs: Vec::new(),
            node_role: "Unknown".to_string(),
            start_block: None,
            skip_sync: false,
            is_paused: false,
            harness_mode: false,
            harness_logs: Vec::new(),
            harness_block: 0,
            harness_init_block: 0,
            l1_blocks: Vec::new(),
            l1_head: 0,
            latest_batch_submission_l1_block: None,
            latest_derivation_origin_l1_block: None,
            batch_to_l1_block: HashMap::new(),
        }
    }

    /// Reset all state to initial values.
    ///
    /// This clears all logs, metrics, and statistics, returning the app to
    /// its initial state. The pause state is not affected.
    pub fn reset(&mut self) {
        self.unsafe_head = 0;
        self.safe_head = 0;
        self.finalized_head = 0;
        self.sync_state = SyncState::Idle;
        self.syncs_completed = 0;
        self.sync_start_time = None;
        self.blocks_processed = 0;
        self.batches_submitted = 0;
        self.batches_derived = 0;
        self.transactions_received = 0;
        self.tx_tracking_start_time = None;
        self.blocks_fetched = 0;
        self.blocks_executed = 0;
        self.last_fetched_block = 0;
        self.last_executed_block = 0;
        self.execution_times.clear();
        self.total_gas_used = 0;
        self.recent_gas_used.clear();
        self.execution_start_time = None;
        self.last_execution_time = None;
        self.execution_logs.clear();
        self.batch_submit_times.clear();
        self.round_trip_latencies.clear();
        self.compression_ratios.clear();
        self.sync_logs.clear();
        self.block_builder_logs.clear();
        self.batch_logs.clear();
        self.derivation_logs.clear();
        self.derivation_execution_logs.clear();
        self.harness_logs.clear();
        self.harness_block = 0;
        self.harness_init_block = 0;
        self.l1_blocks.clear();
        self.l1_head = 0;
        self.latest_batch_submission_l1_block = None;
        self.latest_derivation_origin_l1_block = None;
        self.batch_to_l1_block.clear();
    }

    /// Toggle the pause state.
    ///
    /// When paused, the TUI will not process new events or update the display.
    pub const fn toggle_pause(&mut self) {
        self.is_paused = !self.is_paused;
    }

    /// Update node mode information.
    ///
    /// # Arguments
    ///
    /// * `node_role` - The node role (Sequencer, Validator, or Dual)
    /// * `start_block` - The starting block number (None = from checkpoint)
    /// * `skip_sync` - Whether sync stage was skipped
    pub fn set_mode_info(&mut self, node_role: String, start_block: Option<u64>, skip_sync: bool) {
        self.node_role = node_role;
        self.start_block = start_block;
        self.skip_sync = skip_sync;
    }

    /// Add a log entry to the sync logs.
    ///
    /// Log entries are kept in a circular buffer with a maximum of 100 entries.
    ///
    /// # Arguments
    ///
    /// * `entry` - The log entry to add
    pub fn log_sync(&mut self, entry: LogEntry) {
        self.sync_logs.push(entry);
        if self.sync_logs.len() > MAX_LOG_ENTRIES {
            self.sync_logs.remove(0);
        }
    }

    /// Start a sync operation.
    ///
    /// Records the start time and sets the sync state to Syncing.
    pub fn start_sync(&mut self, start_block: u64, target_block: u64) {
        self.sync_state = SyncState::Syncing {
            start_block,
            current_block: start_block,
            target_block,
            blocks_per_second: 0.0,
            eta: None,
        };
        self.sync_start_time = Some(Instant::now());
    }

    /// Update sync progress.
    ///
    /// If not already syncing, this will start a sync using the current_block
    /// as an approximation for the start block.
    pub const fn update_sync_progress(
        &mut self,
        current_block: u64,
        target_block: u64,
        blocks_per_second: f64,
        eta: Option<Duration>,
    ) {
        // Preserve start_block if already syncing, otherwise use current_block as approximation
        let start_block = match self.sync_state {
            SyncState::Syncing { start_block, .. } => start_block,
            SyncState::Idle | SyncState::Completed { .. } => current_block,
        };

        self.sync_state =
            SyncState::Syncing { start_block, current_block, target_block, blocks_per_second, eta };
    }

    /// Complete a sync operation.
    pub const fn complete_sync(&mut self, blocks_synced: u64, duration_secs: f64) {
        self.sync_state = SyncState::Completed { blocks_synced, duration_secs };
        self.syncs_completed += 1;
        self.sync_start_time = None;
    }

    /// Get the elapsed sync time in seconds.
    pub fn sync_elapsed_secs(&self) -> f64 {
        self.sync_start_time.map_or(0.0, |t| t.elapsed().as_secs_f64())
    }

    /// Check if sync is complete (either Completed state or Idle with blocks synced).
    pub const fn is_sync_complete(&self) -> bool {
        matches!(self.sync_state, SyncState::Completed { .. })
    }

    /// Get sync progress as a percentage (0.0 to 100.0).
    pub fn sync_progress_percent(&self) -> f64 {
        match self.sync_state {
            SyncState::Syncing { start_block, current_block, target_block, .. } => {
                if target_block <= start_block {
                    100.0
                } else {
                    let total = (target_block - start_block) as f64;
                    let done = (current_block.saturating_sub(start_block)) as f64;
                    (done / total) * 100.0
                }
            }
            SyncState::Completed { .. } => 100.0,
            SyncState::Idle => 0.0,
        }
    }

    /// Add a log entry to the block builder logs.
    ///
    /// Log entries are kept in a circular buffer with a maximum of 100 entries.
    ///
    /// # Arguments
    ///
    /// * `entry` - The log entry to add
    pub fn log_block(&mut self, entry: LogEntry) {
        self.block_builder_logs.push(entry);
        if self.block_builder_logs.len() > MAX_LOG_ENTRIES {
            self.block_builder_logs.remove(0);
        }
    }

    /// Add a log entry to the batch submission logs.
    ///
    /// Log entries are kept in a circular buffer with a maximum of 100 entries.
    ///
    /// # Arguments
    ///
    /// * `entry` - The log entry to add
    pub fn log_batch(&mut self, entry: LogEntry) {
        self.batch_logs.push(entry);
        if self.batch_logs.len() > MAX_LOG_ENTRIES {
            self.batch_logs.remove(0);
        }
    }

    /// Add a log entry to the derivation logs.
    ///
    /// Log entries are kept in a circular buffer with a maximum of 100 entries.
    ///
    /// # Arguments
    ///
    /// * `entry` - The log entry to add
    pub fn log_derivation(&mut self, entry: LogEntry) {
        self.derivation_logs.push(entry);
        if self.derivation_logs.len() > MAX_LOG_ENTRIES {
            self.derivation_logs.remove(0);
        }
    }

    /// Add a log entry to the derivation execution logs.
    ///
    /// Log entries are kept in a circular buffer with a maximum of 100 entries.
    ///
    /// # Arguments
    ///
    /// * `entry` - The log entry to add
    pub fn log_derivation_execution(&mut self, entry: LogEntry) {
        self.derivation_execution_logs.push(entry);
        if self.derivation_execution_logs.len() > MAX_LOG_ENTRIES {
            self.derivation_execution_logs.remove(0);
        }
    }

    /// Update the unsafe head.
    ///
    /// The unsafe head represents the latest L2 block received from the sequencer.
    ///
    /// # Arguments
    ///
    /// * `head` - The new unsafe head block number
    pub fn set_unsafe_head(&mut self, head: u64) {
        self.unsafe_head = head;
        self.blocks_processed = self.blocks_processed.max(head);
    }

    /// Update the safe head.
    ///
    /// The safe head represents the latest L2 block that has been submitted to L1.
    ///
    /// # Arguments
    ///
    /// * `head` - The new safe head block number
    pub const fn set_safe_head(&mut self, head: u64) {
        self.safe_head = head;
    }

    /// Update the finalized head.
    ///
    /// The finalized head represents the latest L2 block that has been successfully
    /// re-derived and executed from L1 data.
    ///
    /// # Arguments
    ///
    /// * `head` - The new finalized head block number
    pub const fn set_finalized_head(&mut self, head: u64) {
        self.finalized_head = head;
    }

    /// Record transactions received by the sequencer.
    ///
    /// This tracks the total number of transactions seen in built blocks,
    /// used to calculate the transactions per second rate.
    ///
    /// # Arguments
    ///
    /// * `tx_count` - Number of transactions received
    pub fn record_transactions_received(&mut self, tx_count: usize) {
        // Start timing on first transaction
        if self.tx_tracking_start_time.is_none() && tx_count > 0 {
            self.tx_tracking_start_time = Some(Instant::now());
        }
        self.transactions_received += tx_count as u64;
    }

    /// Get transactions per second rate.
    ///
    /// Returns the rate of transactions received by the sequencer.
    pub fn transactions_per_second(&self) -> f64 {
        self.tx_tracking_start_time.map_or(0.0, |start| {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 { self.transactions_received as f64 / elapsed } else { 0.0 }
        })
    }

    /// Record a batch submission time.
    ///
    /// This is used to track the round-trip latency from batch submission to
    /// re-derivation. The batch number is stored with the current time.
    ///
    /// # Arguments
    ///
    /// * `batch_number` - The batch number being submitted
    pub fn record_batch_submit(&mut self, batch_number: u64) {
        self.batch_submit_times.insert(batch_number, Instant::now());
        self.batches_submitted += 1;
    }

    /// Record a batch re-derivation and calculate latency.
    ///
    /// If the batch submission time was recorded, this calculates the round-trip
    /// latency and adds it to the latency buffer. The submission time is removed
    /// from the tracking map.
    ///
    /// # Arguments
    ///
    /// * `batch_number` - The batch number being re-derived
    pub fn record_batch_derived(&mut self, batch_number: u64) {
        if let Some(submit_time) = self.batch_submit_times.remove(&batch_number) {
            let latency_ms = submit_time.elapsed().as_millis() as u64;
            self.record_latency(latency_ms);
        }
        self.batches_derived += 1;
    }

    /// Record a round-trip latency measurement.
    ///
    /// Latencies are kept in a circular buffer. The maximum number of entries
    /// is 1000.
    ///
    /// # Arguments
    ///
    /// * `latency_ms` - The latency in milliseconds
    pub fn record_latency(&mut self, latency_ms: u64) {
        self.round_trip_latencies.push(latency_ms);
        if self.round_trip_latencies.len() > 1000 {
            self.round_trip_latencies.remove(0);
        }
    }

    /// Calculate the average round-trip latency.
    ///
    /// Returns the average latency in milliseconds, or 0.0 if no latencies
    /// have been recorded.
    pub fn avg_latency_ms(&self) -> f64 {
        if self.round_trip_latencies.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.round_trip_latencies.iter().sum();
        sum as f64 / self.round_trip_latencies.len() as f64
    }

    /// Calculate the standard deviation of round-trip latency.
    ///
    /// Returns the standard deviation in milliseconds, or 0.0 if fewer than
    /// 2 latencies have been recorded.
    pub fn latency_stddev_ms(&self) -> f64 {
        if self.round_trip_latencies.len() < 2 {
            return 0.0;
        }

        let avg = self.avg_latency_ms();
        let variance = self
            .round_trip_latencies
            .iter()
            .map(|&x| {
                let diff = x as f64 - avg;
                diff * diff
            })
            .sum::<f64>()
            / self.round_trip_latencies.len() as f64;

        variance.sqrt()
    }

    /// Record a compression ratio.
    ///
    /// Ratios are kept in a circular buffer. The maximum number of entries
    /// is 1000.
    ///
    /// # Arguments
    ///
    /// * `ratio` - The compression ratio as a percentage (compressed/uncompressed * 100)
    pub fn record_compression_ratio(&mut self, ratio: f64) {
        self.compression_ratios.push(ratio);
        if self.compression_ratios.len() > 1000 {
            self.compression_ratios.remove(0);
        }
    }

    /// Calculate the average compression ratio.
    ///
    /// Returns the average compression ratio as a percentage, or 0.0 if no ratios
    /// have been recorded.
    pub fn avg_compression_ratio(&self) -> f64 {
        if self.compression_ratios.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.compression_ratios.iter().sum();
        sum / self.compression_ratios.len() as f64
    }

    /// Calculate the standard deviation of compression ratio.
    ///
    /// Returns the standard deviation as a percentage, or 0.0 if fewer than
    /// 2 ratios have been recorded.
    pub fn compression_stddev(&self) -> f64 {
        if self.compression_ratios.len() < 2 {
            return 0.0;
        }

        let avg = self.avg_compression_ratio();
        let variance = self
            .compression_ratios
            .iter()
            .map(|&x| {
                let diff = x - avg;
                diff * diff
            })
            .sum::<f64>()
            / self.compression_ratios.len() as f64;

        variance.sqrt()
    }

    /// Record a block execution.
    ///
    /// Updates execution tracking metrics and logs the execution.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block number that was executed
    /// * `execution_time_us` - Time taken to execute the block in microseconds
    /// * `gas_used` - Gas used by the block
    pub fn record_block_executed(
        &mut self,
        block_number: u64,
        execution_time_us: u64,
        gas_used: u64,
    ) {
        self.blocks_executed += 1;
        self.last_executed_block = block_number;

        // Start timing on first execution
        if self.execution_start_time.is_none() {
            self.execution_start_time = Some(Instant::now());
        }

        // Track when this execution happened
        self.last_execution_time = Some(Instant::now());

        // Track execution times in microseconds for sub-millisecond precision
        self.execution_times.push(execution_time_us);
        if self.execution_times.len() > 100 {
            self.execution_times.remove(0);
        }

        // Track gas usage for gigagas/s calculation
        self.total_gas_used += gas_used as u128;
        self.recent_gas_used.push(gas_used);
        if self.recent_gas_used.len() > 100 {
            self.recent_gas_used.remove(0);
        }
    }

    /// Update backlog information (blocks fetched from RPC).
    ///
    /// # Arguments
    ///
    /// * `blocks_fetched` - Total blocks fetched since start
    /// * `last_fetched_block` - The last block number fetched
    pub const fn update_backlog(&mut self, blocks_fetched: u64, last_fetched_block: u64) {
        self.blocks_fetched = blocks_fetched;
        self.last_fetched_block = last_fetched_block;
    }

    /// Get the current backlog size (blocks waiting to be executed).
    pub const fn backlog_size(&self) -> u64 {
        self.blocks_fetched.saturating_sub(self.blocks_executed)
    }

    /// Get execution rate in blocks per second.
    pub fn execution_rate(&self) -> f64 {
        self.execution_start_time.map_or(0.0, |start| {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 { self.blocks_executed as f64 / elapsed } else { 0.0 }
        })
    }

    /// Get gas per second (Mgas/s).
    ///
    /// Returns the gas throughput in megagas per second, calculated from
    /// total gas used divided by elapsed time.
    pub fn mgas_per_second(&self) -> f64 {
        self.execution_start_time.map_or(0.0, |start| {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                // Convert gas to megagas (1 Mgas = 1,000,000 gas)
                (self.total_gas_used as f64 / 1_000_000.0) / elapsed
            } else {
                0.0
            }
        })
    }

    /// Get average execution time per block in milliseconds.
    ///
    /// Internally stores times in microseconds for sub-millisecond precision,
    /// then converts to milliseconds for display.
    pub fn avg_execution_time_ms(&self) -> f64 {
        if self.execution_times.is_empty() {
            return 0.0;
        }
        // execution_times stores microseconds, convert to milliseconds
        let sum_us: u64 = self.execution_times.iter().sum();
        let avg_us = sum_us as f64 / self.execution_times.len() as f64;
        avg_us / 1000.0
    }

    /// Get time since last block execution in seconds.
    ///
    /// Returns None if no blocks have been executed yet.
    pub fn time_since_last_execution(&self) -> Option<Duration> {
        self.last_execution_time.map(|t| t.elapsed())
    }

    /// Add a log entry to the execution logs.
    ///
    /// Log entries are kept in a circular buffer with a maximum of 100 entries.
    ///
    /// # Arguments
    ///
    /// * `entry` - The log entry to add
    pub fn log_execution(&mut self, entry: LogEntry) {
        self.execution_logs.push(entry);
        if self.execution_logs.len() > MAX_LOG_ENTRIES {
            self.execution_logs.remove(0);
        }
    }

    /// Add a log entry to the harness logs.
    ///
    /// Log entries are kept in a circular buffer with a maximum of 100 entries.
    ///
    /// # Arguments
    ///
    /// * `entry` - The log entry to add
    pub fn log_harness(&mut self, entry: LogEntry) {
        self.harness_logs.push(entry);
        if self.harness_logs.len() > MAX_LOG_ENTRIES {
            self.harness_logs.remove(0);
        }
    }

    /// Record a harness block production.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block number produced
    /// * `tx_count` - Number of transactions in the block
    pub fn record_harness_block(&mut self, block_number: u64, tx_count: usize) {
        self.harness_block = block_number;
        self.log_harness(LogEntry::info(format!("Block #{} ({} txs)", block_number, tx_count)));
    }

    /// Mark harness initialization as complete at the given block.
    ///
    /// After this is called, only blocks > harness_init_block will be logged
    /// to avoid re-printing blocks that were shown during initialization.
    pub const fn complete_harness_init(&mut self, final_block: u64) {
        self.harness_init_block = final_block;
    }

    /// Check if a block is after harness initialization.
    ///
    /// Returns true if the block is newer than what was logged during init,
    /// meaning it should be logged to the harness activity panel.
    pub const fn is_after_harness_init(&self, block_number: u64) -> bool {
        block_number > self.harness_init_block
    }

    /// Record a new L1 block.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The L1 block number
    pub fn record_l1_block(&mut self, block_number: u64) {
        // Only add if it's a new block
        if block_number > self.l1_head {
            self.l1_head = block_number;
            self.l1_blocks.push(L1Block {
                number: block_number,
                batch_number: None,
                batch_block_count: None,
                is_derivation_origin: false,
            });

            // Keep the buffer bounded
            if self.l1_blocks.len() > MAX_L1_BLOCKS {
                self.l1_blocks.remove(0);
            }
        }
    }

    /// Record a batch submission in an L1 block.
    ///
    /// If the L1 block doesn't exist yet, it will be created.
    ///
    /// # Arguments
    ///
    /// * `l1_block_number` - The L1 block that contains the batch
    /// * `batch_number` - The batch number submitted
    /// * `block_count` - Number of L2 blocks in the batch
    pub fn record_batch_in_l1_block(
        &mut self,
        l1_block_number: u64,
        batch_number: u64,
        block_count: usize,
    ) {
        // Update latest batch submission L1 block
        self.latest_batch_submission_l1_block = Some(l1_block_number);

        // Record the mapping from batch to L1 block for later derivation origin lookup
        self.batch_to_l1_block.insert(batch_number, l1_block_number);

        // First, ensure the L1 block exists
        if l1_block_number > self.l1_head {
            self.l1_head = l1_block_number;
            self.l1_blocks.push(L1Block {
                number: l1_block_number,
                batch_number: Some(batch_number),
                batch_block_count: Some(block_count),
                is_derivation_origin: false,
            });

            if self.l1_blocks.len() > MAX_L1_BLOCKS {
                self.l1_blocks.remove(0);
            }
        } else {
            // Update existing block if found
            if let Some(block) = self.l1_blocks.iter_mut().find(|b| b.number == l1_block_number) {
                block.batch_number = Some(batch_number);
                block.batch_block_count = Some(block_count);
            }
        }
    }

    /// Record that a batch was derived from a specific L1 block.
    ///
    /// This updates the derivation origin cursor to point at the L1 block
    /// that contained the batch data.
    ///
    /// # Arguments
    ///
    /// * `l1_block_number` - The L1 block that was the origin of the derived batch
    pub fn record_derivation_origin(&mut self, l1_block_number: u64) {
        // Update latest derivation origin L1 block
        self.latest_derivation_origin_l1_block = Some(l1_block_number);

        // Mark the L1 block as a derivation origin
        if let Some(block) = self.l1_blocks.iter_mut().find(|b| b.number == l1_block_number) {
            block.is_derivation_origin = true;
        }
    }

    /// Record that a batch was derived and update the derivation origin cursor.
    ///
    /// Looks up which L1 block the batch was submitted in and updates
    /// the derivation origin cursor to point at that block.
    ///
    /// # Arguments
    ///
    /// * `batch_number` - The batch number that was derived
    pub fn record_batch_derivation_origin(&mut self, batch_number: u64) {
        // Look up the L1 block where this batch was submitted
        if let Some(&l1_block) = self.batch_to_l1_block.get(&batch_number) {
            self.record_derivation_origin(l1_block);
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_new() {
        let app = App::new();
        assert_eq!(app.unsafe_head, 0);
        assert_eq!(app.safe_head, 0);
        assert_eq!(app.finalized_head, 0);
        assert_eq!(app.blocks_processed, 0);
        assert!(!app.is_paused);
    }

    #[test]
    fn test_app_reset() {
        let mut app = App::new();
        app.unsafe_head = 100;
        app.blocks_processed = 100;
        app.batches_submitted = 10;

        app.reset();

        assert_eq!(app.unsafe_head, 0);
        assert_eq!(app.blocks_processed, 0);
        assert_eq!(app.batches_submitted, 0);
    }

    #[test]
    fn test_toggle_pause() {
        let mut app = App::new();
        assert!(!app.is_paused);

        app.toggle_pause();
        assert!(app.is_paused);

        app.toggle_pause();
        assert!(!app.is_paused);
    }

    #[test]
    fn test_latency_metrics() {
        let mut app = App::new();

        app.record_latency(100);
        app.record_latency(200);
        app.record_latency(300);

        assert_eq!(app.avg_latency_ms(), 200.0);

        // Standard deviation should be approximately 81.65
        let stddev = app.latency_stddev_ms();
        assert!(stddev > 81.0 && stddev < 82.0);
    }
}
