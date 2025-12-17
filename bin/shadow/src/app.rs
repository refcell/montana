//! Application state for the shadow TUI.
//!
//! This module contains the shared state between the TUI and the
//! background batch submission and derivation tasks.

use std::{collections::HashMap, time::Instant};

use montana_batch_context::BatchSubmissionMode;

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
    /// Batch derivation latencies in milliseconds (for calculating avg/variance).
    pub(crate) derivation_latencies_ms: Vec<u64>,
}

impl Stats {
    /// Record a new derivation latency sample.
    pub(crate) fn record_latency(&mut self, latency_ms: u64) {
        // Keep at most the last 100 samples
        if self.derivation_latencies_ms.len() >= 100 {
            self.derivation_latencies_ms.remove(0);
        }
        self.derivation_latencies_ms.push(latency_ms);
    }

    /// Get the average derivation latency in milliseconds.
    pub(crate) fn avg_latency_ms(&self) -> Option<f64> {
        if self.derivation_latencies_ms.is_empty() {
            return None;
        }
        let sum: u64 = self.derivation_latencies_ms.iter().sum();
        Some(sum as f64 / self.derivation_latencies_ms.len() as f64)
    }

    /// Get the variance of derivation latencies in milliseconds squared.
    pub(crate) fn latency_variance_ms(&self) -> Option<f64> {
        let avg = self.avg_latency_ms()?;
        if self.derivation_latencies_ms.len() < 2 {
            return None;
        }
        let variance: f64 = self
            .derivation_latencies_ms
            .iter()
            .map(|&x| {
                let diff = x as f64 - avg;
                diff * diff
            })
            .sum::<f64>()
            / (self.derivation_latencies_ms.len() - 1) as f64;
        Some(variance)
    }

    /// Get the standard deviation of derivation latencies in milliseconds.
    pub(crate) fn latency_stddev_ms(&self) -> Option<f64> {
        self.latency_variance_ms().map(|v| v.sqrt())
    }
}

/// Main application state.
#[derive(Debug)]
pub(crate) struct App {
    /// RPC URL for the chain.
    pub(crate) rpc_url: String,
    /// Compression algorithm name.
    pub(crate) compression: String,
    /// Batch submission mode.
    pub(crate) submission_mode: BatchSubmissionMode,
    /// Anvil endpoint URL (if in Anvil mode).
    pub(crate) anvil_endpoint: Option<String>,
    /// Whether the simulation is paused.
    pub(crate) is_paused: bool,
    /// Statistics for the header.
    pub(crate) stats: Stats,
    /// Batch submission log entries.
    pub(crate) batch_logs: Vec<LogEntry>,
    /// Derivation log entries.
    pub(crate) derivation_logs: Vec<LogEntry>,
    /// Maximum log entries to keep per pane.
    max_logs: usize,
    /// Batch submission timestamps (batch_number -> submit time).
    pub(crate) batch_submission_times: HashMap<u64, Instant>,
}

impl App {
    /// Create a new application state.
    pub(crate) fn new(
        rpc_url: String,
        compression: String,
        submission_mode: BatchSubmissionMode,
        anvil_endpoint: Option<String>,
    ) -> Self {
        let mut app = Self {
            rpc_url,
            compression,
            submission_mode,
            anvil_endpoint: anvil_endpoint.clone(),
            is_paused: false,
            stats: Stats { derivation_healthy: true, ..Default::default() },
            batch_logs: Vec::new(),
            derivation_logs: Vec::new(),
            max_logs: 1000,
            batch_submission_times: HashMap::new(),
        };

        app.batch_logs.push(LogEntry::info("Connecting to RPC..."));

        // Log the submission mode
        match submission_mode {
            BatchSubmissionMode::InMemory => {
                app.batch_logs.push(LogEntry::info("Using in-memory batch queue"));
                app.derivation_logs.push(LogEntry::info("Waiting for batches (in-memory)..."));
            }
            BatchSubmissionMode::Anvil => {
                if let Some(ref endpoint) = anvil_endpoint {
                    app.batch_logs.push(LogEntry::info(format!("Anvil started at {}", endpoint)));
                }
                app.derivation_logs.push(LogEntry::info("Waiting for batches (anvil)..."));
            }
            BatchSubmissionMode::Remote => {
                app.batch_logs.push(LogEntry::warn("Remote mode is not yet supported"));
                app.derivation_logs.push(LogEntry::warn("Remote mode is not yet supported"));
            }
        }

        app
    }

    /// Reset the application state.
    pub(crate) fn reset(&mut self) {
        self.stats = Stats { derivation_healthy: true, ..Default::default() };
        self.batch_logs.clear();
        self.derivation_logs.clear();
        self.batch_submission_times.clear();
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

    /// Update the chain head.
    pub(crate) const fn set_chain_head(&mut self, head: u64) {
        self.stats.chain_head = head;
    }

    /// Update the current block being processed.
    pub(crate) const fn set_current_block(&mut self, block: u64) {
        self.stats.current_block = block;
    }
}
