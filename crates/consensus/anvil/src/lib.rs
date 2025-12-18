#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod manager;
pub use manager::{AnvilConfig, AnvilManager};

mod sink;
pub use sink::AnvilBatchSink;

mod source;
pub use source::AnvilBatchSource;

mod error;
// Re-export useful types from alloy for convenience
pub use alloy::primitives::Address;
pub use error::AnvilError;
