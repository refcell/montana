#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod sequencer;
pub use sequencer::{Sequencer, SequencerEvent};

pub mod validator;
pub use validator::{Validator, ValidatorEvent};

/// Result of a single role tick.
///
/// Indicates what happened during a role's tick operation, allowing the
/// node to make decisions about scheduling and resource allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickResult {
    /// Made progress, continue ticking.
    ///
    /// The role performed useful work and may have more work to do.
    /// The node should continue calling tick without delay.
    Progress,
    /// No work available, can idle.
    ///
    /// The role has no work to do right now. The node may wait before
    /// calling tick again to avoid spinning.
    Idle,
    /// Role has completed (e.g., finished historical range).
    ///
    /// The role has finished all its work and will not produce more
    /// progress. The node may stop calling tick for this role.
    Complete,
}

/// Checkpoint data specific to a role.
///
/// Contains role-specific state that can be persisted and restored
/// across node restarts. Each field is optional to support different
/// role types (sequencer vs validator).
#[derive(Debug, Clone, Default)]
pub struct RoleCheckpoint {
    /// For sequencer: last batch submitted.
    ///
    /// The batch number of the last batch successfully submitted to L1.
    pub last_batch_submitted: Option<u64>,
    /// For validator: last batch derived.
    ///
    /// The batch number of the last batch successfully derived and validated.
    pub last_batch_derived: Option<u64>,
    /// Last block processed by this role.
    ///
    /// The L2 block number of the last block this role has processed.
    pub last_block_processed: Option<u64>,
}

/// A role that the node can operate as during the active stage.
///
/// Roles define the primary behavior of a node during its active phase.
/// The two main roles are:
/// - Sequencer: Executes transactions and submits batches to L1
/// - Validator: Derives and validates batches from L1
///
/// Roles implement a tick-based execution model where the node repeatedly
/// calls `tick()` to drive progress.
#[async_trait::async_trait]
pub trait Role: Send + Sync {
    /// Human-readable name for logging.
    ///
    /// Returns a static string identifying this role type (e.g., "sequencer" or "validator").
    fn name(&self) -> &'static str;

    /// Resume from a checkpoint after restart.
    ///
    /// Called when the node starts up to restore the role's state from a
    /// previously saved checkpoint. The role should restore its internal state
    /// to continue from where it left off.
    ///
    /// # Arguments
    /// * `checkpoint` - The checkpoint data to resume from
    ///
    /// # Errors
    /// Returns an error if the checkpoint data is invalid or the role cannot
    /// be properly resumed.
    async fn resume(&mut self, checkpoint: &montana_checkpoint::Checkpoint) -> eyre::Result<()>;

    /// Run one iteration of the role's main loop.
    ///
    /// Performs a single unit of work for this role. The implementation should
    /// be non-blocking and return quickly to allow the node to multiplex between
    /// roles and other operations.
    ///
    /// # Returns
    /// A `TickResult` indicating what happened:
    /// - `Progress`: Work was done, call tick again soon
    /// - `Idle`: No work available, can wait before next tick
    /// - `Complete`: Role has finished all work
    ///
    /// # Errors
    /// Returns an error if a fatal problem occurs that prevents the role from
    /// continuing. Transient errors should be handled internally when possible.
    async fn tick(&mut self) -> eyre::Result<TickResult>;

    /// Get current checkpoint state for this role.
    ///
    /// Returns the current state that should be persisted to allow resuming
    /// this role after a restart. Called periodically by the node to save
    /// progress.
    fn checkpoint(&self) -> RoleCheckpoint;
}
