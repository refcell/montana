#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod batch_header;
mod compressed_batch;
mod l2_block_data;
mod submission_receipt;

pub use alloy_primitives::Bytes;
pub use batch_header::BatchHeader;
pub use compressed_batch::CompressedBatch;
pub use l2_block_data::L2BlockData;
pub use submission_receipt::SubmissionReceipt;
