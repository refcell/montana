#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod config;
pub use config::{DerivationConfig, DerivationConfigBuilder};

mod runner;
pub use runner::DerivationRunner;

mod metrics;
pub use metrics::DerivationMetrics;

mod error;
pub use error::DerivationError;
pub use montana_checkpoint::{Checkpoint, CheckpointError};
