use std::path::PathBuf;

use clap::Parser;
use montana_batcher::BatchSubmissionMode;

use crate::MontanaMode;

/// Montana node CLI arguments.
///
/// The node can operate in two modes based on the `--skip-sync` flag:
/// - **Sync mode** (default): Catches up to the chain tip before processing
/// - **Skip-sync mode**: Starts processing immediately without catching up
///
/// Block range can be specified with `--start` and `--end`:
/// - If both are specified: Process that specific block range
/// - If only `--start`: Process from start block to chain tip (and follow)
/// - If neither: Process from checkpoint (or genesis) to chain tip (and follow)
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
    /// blocks directly. Use with --start/--end to re-execute a specific
    /// block range without sync overhead.
    #[arg(long)]
    pub skip_sync: bool,

    /// Starting block number (inclusive).
    ///
    /// If specified, processing starts from this block.
    /// If not specified, starts from checkpoint or genesis.
    #[arg(long)]
    pub start: Option<u64>,

    /// Ending block number (inclusive).
    ///
    /// If specified, processing stops after this block.
    /// If not specified, follows the chain tip indefinitely.
    #[arg(long)]
    pub end: Option<u64>,

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

impl MontanaCli {
    /// Returns true if this is a bounded block range (has an end block).
    #[must_use]
    pub const fn is_bounded_range(&self) -> bool {
        self.end.is_some()
    }

    /// Returns the block range description for display.
    #[must_use]
    pub fn range_description(&self) -> String {
        match (self.start, self.end) {
            (Some(start), Some(end)) => format!("{}-{}", start, end),
            (Some(start), None) => format!("{}->tip", start),
            (None, Some(end)) => format!("0-{}", end),
            (None, None) => "tip".to_string(),
        }
    }
}
