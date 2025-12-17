#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod source;
pub use source::{BatchSource, L1BatchSource, L2BlockData, RawTransaction, SourceError};

mod execute;
pub use execute::{ExecutePayload, ExecutePayloadError, NoopExecutor};

mod sink;
pub use sink::{BatchSink, CompressedBatch, L2BlockSink, SinkError, SubmissionReceipt};

mod compressor;
pub use compressor::{CompressionConfig, CompressionError, Compressor};

mod codec;
pub use codec::{BatchCodec, BatchHeader, CodecError};

mod error;
pub use error::PipelineError;

pub mod constants;
