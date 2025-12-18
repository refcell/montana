#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// Used in tests
#[cfg(not(test))]
use async_trait as _;
#[cfg(not(test))]
use tracing as _;

mod config;
pub use config::{DerivationConfig, DerivationConfigBuilder};

mod runner;
pub use runner::DerivationRunner;

mod metrics;
pub use metrics::DerivationMetrics;

mod error;
pub use error::DerivationError;
pub use montana_checkpoint::{Checkpoint, CheckpointError};
