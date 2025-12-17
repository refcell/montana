//! Batcher service for L2 batch submission orchestration.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// Used in tests and future implementation
#[cfg(not(test))]
use montana_txmgr as _;

mod config;
pub use config::{BatcherConfig, BatcherConfigBuilder};

mod context;
pub use context::{BatchContext, BatchContextError, BatchSink, InMemoryBatchSink};

mod driver;
pub use driver::{BatchDriver, PendingBatch};

mod error;
pub use error::BatcherError;

mod metrics;
pub use metrics::BatcherMetrics;
// Re-export BatchSubmissionMode from montana-cli
pub use montana_cli::BatchSubmissionMode;

mod service;
pub use service::{BatcherService, BatcherState};
// Re-export useful types from dependencies
pub use montana_anvil::Address;
pub use montana_checkpoint::{Checkpoint, CheckpointError};
