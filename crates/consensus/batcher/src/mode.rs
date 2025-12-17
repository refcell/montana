//! Batch submission mode for sequencer operations.
//!
//! This module defines the different modes for submitting batches
//! during sequencer execution.

use clap::ValueEnum;

/// Batch submission mode for the sequencer.
///
/// Defines how compressed batches are submitted during sequencer execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum BatchSubmissionMode {
    /// In-memory batch queue.
    ///
    /// Batches are logged but not actually submitted anywhere.
    /// Useful for testing execution without submission overhead.
    InMemory,

    /// Local Anvil chain (default).
    ///
    /// Spawns a local Anvil instance and submits batches as transactions.
    /// Provides realistic simulation of L1 submission without remote dependencies.
    #[default]
    Anvil,

    /// Remote L1 chain submission (not yet implemented).
    ///
    /// Submits batches to a remote L1 chain. Currently unsupported.
    Remote,
}

impl std::fmt::Display for BatchSubmissionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InMemory => write!(f, "in-memory"),
            Self::Anvil => write!(f, "anvil"),
            Self::Remote => write!(f, "remote"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_submission_mode_default() {
        assert_eq!(BatchSubmissionMode::default(), BatchSubmissionMode::Anvil);
    }

    #[test]
    fn batch_submission_mode_display() {
        assert_eq!(BatchSubmissionMode::InMemory.to_string(), "in-memory");
        assert_eq!(BatchSubmissionMode::Anvil.to_string(), "anvil");
        assert_eq!(BatchSubmissionMode::Remote.to_string(), "remote");
    }

    #[test]
    fn batch_submission_mode_clone() {
        let mode = BatchSubmissionMode::Anvil;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    #[test]
    fn batch_submission_mode_debug() {
        let debug = format!("{:?}", BatchSubmissionMode::Anvil);
        assert!(debug.contains("Anvil"));
    }
}
