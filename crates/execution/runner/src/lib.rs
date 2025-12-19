#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use blocksource::OpBlock;
use vm::BlockResult;

/// An executed block with its execution result
#[derive(Debug, Clone)]
pub struct ExecutedBlock {
    /// The original block
    pub block: OpBlock,
    /// The execution result
    pub result: BlockResult,
}

pub mod verification;
