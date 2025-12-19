#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod config;
pub use config::{BatchSubmissionConfig, BatchSubmissionConfigBuilder};

mod source;
pub use source::{BlockSource, BlockSourceError};

mod rpc;
pub use rpc::RpcBlockSource;

mod runner;
pub use runner::{BatchSubmissionCallback, BatchSubmissionRunner};

mod metrics;
pub use metrics::BatchSubmissionMetrics;

mod error;
pub use error::BatchSubmissionError;
