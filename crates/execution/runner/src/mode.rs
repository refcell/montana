//! Producer mode configuration

use std::time::Duration;

/// Producer mode for block fetching
#[derive(Debug, Clone)]
pub enum ProducerMode {
    /// Poll for live blocks
    Live {
        /// Poll interval
        poll_interval: Duration,
        /// Starting block number (default: latest)
        start_block: Option<u64>,
    },
    /// Fetch a historical range of blocks
    Historical {
        /// Starting block number (inclusive)
        start: u64,
        /// Ending block number (inclusive)
        end: u64,
    },
}
