#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub use alloy_primitives::Bytes;

mod batch_header;
pub use batch_header::BatchHeader;

mod compressed_batch;
pub use compressed_batch::CompressedBatch;

mod l2_block_data;
pub use l2_block_data::L2BlockData;

mod submission_receipt;
pub use submission_receipt::SubmissionReceipt;
