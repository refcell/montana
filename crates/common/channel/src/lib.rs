#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod errors;
mod execute;
mod source;

pub use errors::ExecutePayloadError;
pub use execute::{ChannelExecutor, ExecutePayload, NoopExecutor};
pub use source::{BatchSource, L2BlockData, RawTransaction, SourceError};
