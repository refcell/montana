use crate::SyncProgress;

/// The full lifecycle state of the node.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeState {
    /// Node is starting up
    Starting,
    /// Node is syncing to chain tip
    Syncing(SyncProgress),
    /// Node is fully synced and running
    Active,
    /// Node is shutting down
    Stopping,
    /// Node has stopped
    Stopped,
}

impl NodeState {
    /// Returns true if the node is currently syncing.
    pub fn is_syncing(&self) -> bool {
        matches!(self, Self::Syncing(_))
    }

    /// Returns true if the node is fully synced and active.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}
