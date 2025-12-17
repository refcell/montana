use std::path::PathBuf;

use crate::NodeRole;

/// Configuration for the node.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeConfig {
    /// The operational role of the node
    pub role: NodeRole,
    /// Path to the checkpoint file
    pub checkpoint_path: PathBuf,
    /// Interval in seconds between checkpoint saves
    pub checkpoint_interval_secs: u64,
    /// Whether to skip initial sync and start from current state
    pub skip_sync: bool,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            role: NodeRole::default(),
            checkpoint_path: PathBuf::from("./data/checkpoint.json"),
            checkpoint_interval_secs: 10,
            skip_sync: false,
        }
    }
}
