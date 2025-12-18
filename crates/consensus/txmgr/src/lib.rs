#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod builder;
pub use builder::TxBuilder;

pub mod candidate;
pub use candidate::{TxCandidate, TxReceipt};

pub mod config;
pub use config::{TxManagerConfig, TxManagerConfigBuilder};

pub mod error;
pub use error::TxError;

pub mod gas;
pub use gas::{GasCaps, GasOracle};

pub mod monitor;
pub use monitor::TxMonitor;

pub mod nonce;
pub use nonce::NonceTracker;

pub mod state;
pub use state::SendState;

pub mod submitter;
pub use submitter::TxSubmitter;
