use std::{collections::HashMap, time::Instant};

use montana_tui_common::LogEntry;

/// Maximum number of log entries to keep in each log buffer.
const MAX_LOG_ENTRIES: usize = 100;

/// Application state for the Montana TUI.
///
/// This struct maintains all the state needed to render the 4-pane TUI:
/// - Chain head progression (unsafe, safe, finalized)
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

    // Statistics
    /// Total number of L2 blocks processed
    pub blocks_processed: u64,
    /// Total number of batches submitted to L1
    pub batches_submitted: u64,
    /// Total number of batches derived from L1
    pub batches_derived: u64,

    // Round-trip latency tracking
    /// Batch submission timestamps (batch_number -> submit time)
    batch_submit_times: HashMap<u64, Instant>,
    /// Round-trip latencies in milliseconds (submit to re-derive)
    round_trip_latencies: Vec<u64>,

    // Log buffers
    /// Transaction pool logs
    pub tx_pool_logs: Vec<LogEntry>,
    /// Block builder logs
    pub block_builder_logs: Vec<LogEntry>,
    /// Batch submission logs
    pub batch_logs: Vec<LogEntry>,
    /// Derivation logs
    pub derivation_logs: Vec<LogEntry>,

    // UI state
    /// Whether the TUI is paused (no updates)
    pub is_paused: bool,
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
            blocks_processed: 0,
            batches_submitted: 0,
            batches_derived: 0,
            batch_submit_times: HashMap::new(),
            round_trip_latencies: Vec::new(),
            tx_pool_logs: Vec::new(),
            block_builder_logs: Vec::new(),
            batch_logs: Vec::new(),
            derivation_logs: Vec::new(),
            is_paused: false,
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
        self.blocks_processed = 0;
        self.batches_submitted = 0;
        self.batches_derived = 0;
        self.batch_submit_times.clear();
        self.round_trip_latencies.clear();
        self.tx_pool_logs.clear();
        self.block_builder_logs.clear();
        self.batch_logs.clear();
        self.derivation_logs.clear();
    }

    /// Toggle the pause state.
    ///
    /// When paused, the TUI will not process new events or update the display.
    pub const fn toggle_pause(&mut self) {
        self.is_paused = !self.is_paused;
    }

    /// Add a log entry to the transaction pool logs.
    ///
    /// Log entries are kept in a circular buffer with a maximum of 100 entries.
    ///
    /// # Arguments
    ///
    /// * `entry` - The log entry to add
    pub fn log_tx(&mut self, entry: LogEntry) {
        self.tx_pool_logs.push(entry);
        if self.tx_pool_logs.len() > MAX_LOG_ENTRIES {
            self.tx_pool_logs.remove(0);
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
