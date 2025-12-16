//! Application state for the shadow TUI.
//!
//! This module contains the shared state between the TUI and the
//! background batch submission and derivation tasks.

use montana_pipeline::CompressedBatch;

/// Log level for display.
#[derive(Clone, Debug, Default)]
pub(crate) enum LogLevel {
    /// Informational message.
    #[default]
    Info,
    /// Warning message.
    Warn,
    /// Error message.
    Error,
}

/// A log entry for display in the TUI.
#[derive(Clone, Debug)]
pub(crate) struct LogEntry {
    /// The log level.
    pub(crate) level: LogLevel,
    /// The log message.
    pub(crate) message: String,
}

impl LogEntry {
    /// Create a new info log entry.
    pub(crate) fn info(message: impl Into<String>) -> Self {
        Self { level: LogLevel::Info, message: message.into() }
    }

    /// Create a new warning log entry.
    pub(crate) fn warn(message: impl Into<String>) -> Self {
        Self { level: LogLevel::Warn, message: message.into() }
    }

    /// Create a new error log entry.
    pub(crate) fn error(message: impl Into<String>) -> Self {
        Self { level: LogLevel::Error, message: message.into() }
    }
}

/// Statistics for display in the header.
#[derive(Clone, Debug, Default)]
pub(crate) struct Stats {
    /// Current chain head block number.
    pub(crate) chain_head: u64,
    /// Current block being processed.
    pub(crate) current_block: u64,
    /// Number of batches submitted.
    pub(crate) batches_submitted: u64,
    /// Number of blocks processed by batch submission.
    pub(crate) blocks_processed: u64,
    /// Total bytes before compression.
    pub(crate) bytes_original: usize,
    /// Total bytes after compression.
    pub(crate) bytes_compressed: usize,
    /// Compression ratio (compressed / original).
    pub(crate) compression_ratio: f64,
    /// Number of batches derived.
    pub(crate) batches_derived: u64,
    /// Number of blocks derived.
    pub(crate) blocks_derived: u64,
    /// Total bytes decompressed.
    pub(crate) bytes_decompressed: usize,
    /// Whether derivation is healthy (matches batch submission).
    pub(crate) derivation_healthy: bool,
}

/// Main application state.
#[derive(Debug)]
pub(crate) struct App {
    /// RPC URL for the chain.
    pub(crate) rpc_url: String,
    /// Compression algorithm name.
    pub(crate) compression: String,
    /// Whether the simulation is paused.
    pub(crate) is_paused: bool,
    /// Statistics for the header.
    pub(crate) stats: Stats,
    /// Batch submission log entries.
    pub(crate) batch_logs: Vec<LogEntry>,
    /// Derivation log entries.
    pub(crate) derivation_logs: Vec<LogEntry>,
    /// Pending batches for derivation (passed from batch submission).
    pub(crate) pending_batches: Vec<CompressedBatch>,
    /// Maximum log entries to keep per pane.
    max_logs: usize,
}

impl App {
    /// Create a new application state.
    pub(crate) fn new(rpc_url: String, compression: String) -> Self {
        let mut app = Self {
            rpc_url,
            compression,
            is_paused: false,
            stats: Stats { derivation_healthy: true, ..Default::default() },
            batch_logs: Vec::new(),
            derivation_logs: Vec::new(),
            pending_batches: Vec::new(),
            max_logs: 1000,
        };

        app.batch_logs.push(LogEntry::info("Connecting to RPC..."));
        app.derivation_logs.push(LogEntry::info("Waiting for batches..."));

        app
    }

    /// Reset the application state.
    pub(crate) fn reset(&mut self) {
        self.stats = Stats { derivation_healthy: true, ..Default::default() };
        self.batch_logs.clear();
        self.derivation_logs.clear();
        self.pending_batches.clear();
        self.is_paused = false;

        self.batch_logs.push(LogEntry::info("Reset - reconnecting to RPC..."));
        self.derivation_logs.push(LogEntry::info("Reset - waiting for batches..."));
    }

    /// Toggle pause state.
    pub(crate) fn toggle_pause(&mut self) {
        self.is_paused = !self.is_paused;
        let msg = if self.is_paused { "Paused" } else { "Resumed" };
        self.batch_logs.push(LogEntry::info(msg));
        self.derivation_logs.push(LogEntry::info(msg));
    }

    /// Add a batch submission log entry.
    pub(crate) fn log_batch(&mut self, entry: LogEntry) {
        self.batch_logs.push(entry);
        if self.batch_logs.len() > self.max_logs {
            self.batch_logs.remove(0);
        }
    }

    /// Add a derivation log entry.
    pub(crate) fn log_derivation(&mut self, entry: LogEntry) {
        self.derivation_logs.push(entry);
        if self.derivation_logs.len() > self.max_logs {
            self.derivation_logs.remove(0);
        }
    }

    /// Queue a batch for derivation.
    pub(crate) fn queue_batch(&mut self, batch: CompressedBatch) {
        self.pending_batches.push(batch);
    }

    /// Take the next pending batch for derivation.
    pub(crate) fn take_batch(&mut self) -> Option<CompressedBatch> {
        if self.pending_batches.is_empty() { None } else { Some(self.pending_batches.remove(0)) }
    }

    /// Update the chain head.
    pub(crate) fn set_chain_head(&mut self, head: u64) {
        self.stats.chain_head = head;
    }

    /// Update the current block being processed.
    pub(crate) fn set_current_block(&mut self, block: u64) {
        self.stats.current_block = block;
    }
}
