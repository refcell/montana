#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod role;
pub use role::NodeRole;

mod stage;
pub use stage::{NodeStage, SyncProgress};

mod state;
pub use state::NodeState;

mod config;
pub use config::NodeConfig;

mod sync;
pub use sync::{SyncConfig, SyncEvent, SyncStage, SyncStatus};

mod events;
pub use events::NodeEvent;

mod observer;
pub use observer::{AsyncObserver, LoggingObserver, NodeObserver, NoopObserver};

mod builder;
pub use builder::{Node, NodeBuilder};
