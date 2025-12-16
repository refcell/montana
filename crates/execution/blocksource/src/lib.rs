//! Block source abstractions for aggregating blocks from multiple data sources.
//!
//! This crate provides a unified interface for receiving blocks from various sources
//! such as RPC polling, P2P gossip, and Engine API.

pub mod convert;
pub mod producer;
pub mod rpc;
pub mod types;

pub use convert::{block_to_env, tx_to_op_tx};
pub use producer::{BlockProducer, HistoricalRangeProducer, LiveRpcProducer};
pub use rpc::RpcBlockSource;
pub use types::OpBlock;
