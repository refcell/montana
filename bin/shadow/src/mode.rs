//! Batch submission mode for the shadow TUI.
//!
//! This module defines the different modes for submitting batches
//! during the shadow simulation.

use clap::ValueEnum;

/// Batch submission mode for the shadow TUI.
///
/// Defines how compressed batches are submitted and retrieved during
/// the shadow simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub(crate) enum BatchSubmissionMode {
    /// In-memory batch queue.
    ///
    /// Batches are passed directly from batch submission to derivation
    /// via an in-memory queue. Fast but doesn't simulate on-chain behavior.
    InMemory,

    /// Local Anvil chain.
    ///
    /// Spawns a local Anvil instance and submits batches as transactions.
    /// Derivation reads batches from the Anvil chain state. Provides
    /// realistic simulation of L1 submission without remote dependencies.
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
