//! Type aliases for block source types.

use alloy::network::Network;
use op_alloy::network::Optimism;

/// Type alias for the Optimism block type
pub type OpBlock = <Optimism as Network>::BlockResponse;
