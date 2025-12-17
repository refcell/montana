#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod cached;
pub use cached::CachedDatabase;

pub mod errors;

pub mod remote;
pub use remote::RPCDatabase;

pub mod traits;
pub use traits::{Database, DatabaseCommit};

pub mod triedb;
