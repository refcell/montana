use std::path::PathBuf;

use clap::Parser;

use crate::{BatchSubmissionMode, MontanaMode};

/// Montana node CLI arguments.
///
/// The node operates in sync + handoff mode by default:
/// - **Sync stage**: Catches up to the chain tip from the starting block
/// - **Active stage**: Sequencer/Validator roles process new blocks
///
/// Use `--skip-sync` to bypass the sync stage (for re-execution from a specific block).
/// Use `--start` to specify a starting block (defaults to checkpoint or genesis).
#[derive(Parser, Debug, Clone)]
#[command(name = "montana", about = "Montana Base Stack Node")]
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

    /// Skip sync stage and start processing blocks immediately.
    ///
    /// When enabled, the node skips the sync stage and begins processing
    /// blocks directly. Use with --start to re-execute from a specific
    /// block without sync overhead.
    #[arg(long)]
    pub skip_sync: bool,

    /// Starting block number (inclusive).
    ///
    /// If specified, processing starts from this block.
    /// If not specified, starts from checkpoint or genesis.
    #[arg(long)]
    pub start: Option<u64>,

    /// Poll interval in milliseconds when following chain tip.
    #[arg(long, default_value = "2000")]
    pub poll_interval_ms: u64,

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

    /// Batch submission mode
    #[arg(long, default_value = "anvil")]
    pub batch_mode: BatchSubmissionMode,

    /// L1 RPC URL (for remote batch mode)
    #[arg(long, env = "L1_RPC_URL")]
    pub l1_rpc_url: Option<String>,
}
