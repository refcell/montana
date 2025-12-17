use std::path::PathBuf;

use clap::{Parser, Subcommand};
use montana_batcher::BatchSubmissionMode;

use crate::MontanaMode;

/// Montana node CLI arguments.
#[derive(Parser, Debug, Clone)]
#[command(name = "montana", about = "Montana OP Stack Node")]
pub struct MontanaCli {
    /// L2 RPC URL
    #[arg(short = 'r', long, env = "L2_RPC_URL")]
    pub rpc_url: String,

    /// Node operating mode
    #[arg(long, default_value = "dual")]
    pub mode: MontanaMode,

    /// Run without TUI interface (headless mode)
    #[arg(long)]
    pub headless: bool,

    /// Skip sync stage (start active immediately)
    #[arg(long)]
    pub skip_sync: bool,

    /// Checkpoint file path
    #[arg(long, default_value = "./data/checkpoint.json")]
    pub checkpoint_path: PathBuf,

    /// Checkpoint save interval in seconds
    #[arg(long, default_value = "10")]
    pub checkpoint_interval: u64,

    /// Blocks behind tip to consider "synced"
    #[arg(long, default_value = "10")]
    pub sync_threshold: u64,

    /// Log level
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Block producer mode (live or historical)
    #[command(subcommand)]
    pub producer: ProducerMode,

    /// Batch submission mode
    #[arg(long, default_value = "anvil")]
    pub batch_mode: BatchSubmissionMode,

    /// L1 RPC URL (for remote batch mode)
    #[arg(long, env = "L1_RPC_URL")]
    pub l1_rpc_url: Option<String>,
}

/// Block producer mode.
#[derive(Subcommand, Debug, Clone)]
pub enum ProducerMode {
    /// Poll for live blocks
    Live {
        /// Poll interval in milliseconds
        #[arg(long, default_value = "2000")]
        poll_interval_ms: u64,
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
