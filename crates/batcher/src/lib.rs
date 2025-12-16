//! Batcher service for L2 batch submission orchestration.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// Used in tests and future implementation
#[cfg(not(test))]
use async_trait as _;
#[cfg(not(test))]
use montana_txmgr as _;
#[cfg(not(test))]
use tokio as _;
#[cfg(not(test))]
use tracing as _;

mod config;
pub use config::{BatcherConfig, BatcherConfigBuilder};

mod driver;
pub use driver::{BatchDriver, PendingBatch};

mod error;
pub use error::BatcherError;

mod metrics;
pub use metrics::BatcherMetrics;

mod service;
pub use service::{BatcherService, BatcherState};
