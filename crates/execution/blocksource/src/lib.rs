#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod convert;
pub mod producer;
pub mod rpc;
pub mod types;

pub use convert::{block_to_env, tx_to_op_tx};
pub use producer::{BlockProducer, HistoricalRangeProducer, LiveRpcProducer};
pub use rpc::{BlockSource, RpcBlockSource};
pub use types::OpBlock;
