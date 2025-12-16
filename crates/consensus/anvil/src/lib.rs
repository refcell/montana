#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

//! Anvil integration for Montana.
//!
//! This crate provides integration with [Anvil](https://github.com/foundry-rs/foundry/tree/master/crates/anvil),
//! a local Ethereum node for testing and development.
//!
//! # Overview
//!
//! The main components are:
//!
//! - [`AnvilManager`]: Manages the lifecycle of an Anvil instance
//! - [`AnvilBatchSink`]: Submits compressed batches as transactions to Anvil
//! - [`AnvilBatchSource`]: Reads batches from Anvil blocks
//!
//! # Example
//!
//! ```ignore
//! use montana_anvil::{AnvilConfig, AnvilManager};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Spawn Anvil with default config
//!     let anvil = AnvilManager::spawn(AnvilConfig::default()).await?;
//!
//!     // Get sink and source for batch operations
//!     let sink = anvil.sink();
//!     let source = anvil.source();
//!
//!     println!("Anvil running at: {}", anvil.endpoint());
//!     Ok(())
//! }
//! ```

mod manager;
pub use manager::{AnvilConfig, AnvilManager};

mod sink;
pub use sink::AnvilBatchSink;

mod source;
pub use source::AnvilBatchSource;

mod error;
// Re-export useful types from alloy for convenience
pub use alloy::primitives::Address;
pub use error::AnvilError;
