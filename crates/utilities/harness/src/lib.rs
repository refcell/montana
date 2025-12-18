#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

//! Test harness for Montana.
//!
//! This crate provides a self-contained test environment that:
//! - Spawns a local anvil instance as a simulated L2 chain
//! - Generates semi-random transactions using test accounts
//! - Runs in a background thread for continuous activity

mod config;
pub use config::HarnessConfig;

mod harness;
pub use harness::Harness;

mod progress;
pub use progress::{BoxedProgressReporter, HarnessProgressReporter};
