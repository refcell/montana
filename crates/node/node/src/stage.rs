/// The current operational stage of the node.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeStage {
    /// Node is syncing from the network
    Syncing(SyncProgress),
    /// Node is fully synced and actively processing
    Active,
}

/// Progress information during the sync stage.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncProgress {
    /// The block number at which syncing started
    pub start_block: u64,
    /// The current block number being processed
    pub current_block: u64,
    /// The target block number to reach
    pub target_block: u64,
    /// Blocks synced per second (rolling average)
    pub blocks_per_second: f64,
    /// Estimated time remaining
    pub eta: Option<std::time::Duration>,
}

impl SyncProgress {
    /// Returns the sync progress as a percentage between 0.0 and 1.0.
    pub fn progress(&self) -> f64 {
        if self.target_block <= self.start_block {
            return 1.0;
        }

        let total = self.target_block - self.start_block;
        let completed = self.current_block.saturating_sub(self.start_block);

        (completed as f64 / total as f64).min(1.0)
    }

    /// Returns the number of blocks remaining to sync.
    pub fn blocks_remaining(&self) -> u64 {
        self.target_block.saturating_sub(self.current_block)
    }

    /// Returns true if syncing is complete (current block >= target block).
    pub fn is_complete(&self) -> bool {
        self.current_block >= self.target_block
    }
}
