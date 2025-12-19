#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod source;
pub use source::{BatchSource, Bytes, L1BatchSource, L2BlockData, SourceError};

mod sink;
pub use sink::{BatchSink, L2BlockSink, SinkError};

mod compressor;
pub use compressor::{CompressionConfig, CompressionError, Compressor};

mod codec;
pub use codec::{BatchCodec, CodecError};
// Re-export primitives types for backwards compatibility
pub use primitives::{BatchHeader, CompressedBatch, SubmissionReceipt};

mod error;
pub use error::PipelineError;

pub mod constants;
