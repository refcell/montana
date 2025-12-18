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
/// Use `--with-harness` to run with a local test chain (ignores --rpc-url).
#[derive(Parser, Debug, Clone)]
#[command(name = "montana", about = "Montana Base Stack Node")]
pub struct MontanaCli {
    /// L2 RPC URL (ignored when --with-harness is set)
    #[arg(short = 'r', long, env = "L2_RPC_URL")]
    pub rpc_url: Option<String>,

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

    // Harness options
    /// Run with built-in test harness (spawns local anvil chain).
    ///
    /// When enabled, --rpc-url is ignored and the harness anvil URL is used instead.
    /// This is useful for testing and demos without requiring a real RPC endpoint.
    #[arg(long)]
    pub with_harness: bool,

    /// Reset checkpoint before starting (useful for harness mode).
    ///
    /// When enabled, the checkpoint file is deleted before starting,
    /// ensuring a fresh start. Implied by --with-harness.
    #[arg(long)]
    pub reset_checkpoint: bool,

    /// Harness block time in milliseconds.
    ///
    /// Controls how fast blocks are produced in harness mode.
    /// Only used when --with-harness is enabled.
    #[arg(long, default_value = "1000")]
    pub harness_block_time_ms: u64,

    /// Initial blocks to generate before starting (creates sync backlog).
    ///
    /// When > 0, the harness generates this many blocks before Montana starts,
    /// creating a "historical" backlog that must be synced. Useful for testing sync.
    /// Only used when --with-harness is enabled.
    #[arg(long, default_value = "10")]
    pub harness_initial_blocks: u64,
}
