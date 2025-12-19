#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod convert;
pub use convert::{block_to_env, tx_to_op_tx};

pub mod producer;
pub use producer::{BlockProducer, ChannelBlockProducer, RpcBlockProducer};

pub mod rpc;
pub use rpc::{BlockSource, RpcBlockSource};

pub mod types;
pub use types::OpBlock;
