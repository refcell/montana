#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod cached;
pub mod errors;
pub mod remote;
pub mod traits;
pub mod triedb;

pub use cached::CachedDatabase;
pub use errors::DbError;
pub use remote::RPCDatabase;
pub use traits::{DBErrorMarker, Database, DatabaseCommit, DatabaseRef};
