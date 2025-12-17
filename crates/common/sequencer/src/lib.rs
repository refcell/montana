#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod buffer;
mod convert;
mod errors;

pub use buffer::ExecutedBlockBuffer;
pub use convert::{op_block_to_l2_data, tx_to_raw};
pub use errors::BufferError;
// Re-export key types from dependencies for convenience
pub use blocksource::OpBlock;
pub use channels::{BatchSource, L2BlockData, RawTransaction};
