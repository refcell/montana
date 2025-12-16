//! CLI argument definitions for the node binary

use clap::{Parser, Subcommand};

/// CLI arguments
#[derive(Parser, Debug)]
#[command(name = "montana", about = "Execute Base blocks using op-revm")]
pub(crate) struct Args {
    /// RPC URL for the Base node
    #[arg(short, long, env = "BASE_RPC_URL")]
    pub rpc_url: String,

    /// Producer mode
    #[command(subcommand)]
    pub mode: ProducerMode,
}

#[derive(Subcommand, Debug)]
pub(crate) enum ProducerMode {
    /// Poll for live blocks
    Live {
        /// Poll interval in milliseconds
        #[arg(long, default_value = "2000")]
        poll_interval_ms: u64,

        /// Starting block number (default: latest)
        #[arg(long)]
        start_block: Option<u64>,
    },

    /// Fetch a historical range of blocks
    Historical {
        /// Starting block number (inclusive)
        #[arg(long)]
        start: u64,

        /// Ending block number (inclusive)
        #[arg(long)]
        end: u64,
    },
}
