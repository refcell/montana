//! Block execution for OP Stack chains
//!
//! This crate provides a `BlockExecutor` that takes a database and chain specification,
//! then executes blocks and their transactions using op-revm.

mod executor;

pub use executor::{BlockExecutor, BlockResult, ExecutorError, TxResult};
