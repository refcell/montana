#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod source;
pub use source::{JsonBlock, JsonSourceData, JsonTransaction, LocalBatchSource};

mod sink;
pub use sink::{JsonBatch, JsonReceipt, JsonSinkData, LocalBatchSink};

mod compressor;
pub use compressor::NoopCompressor;

mod error;
pub use error::LocalError;
