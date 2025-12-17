//! CLI argument definitions for the node binary

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use montana_batcher::BatchSubmissionMode;
use montana_cli::MontanaMode;

/// CLI arguments
#[derive(Parser, Debug, Clone)]
#[command(name = "montana", about = "Execute Base blocks using op-revm")]
pub(crate) struct Args {
    /// RPC URL for the Base node
    #[arg(short, long, env = "BASE_RPC_URL")]
    pub rpc_url: String,

    /// Operating mode: executor, sequencer, validator, or dual (default)
    #[arg(long, default_value = "dual")]
    pub mode: MontanaMode,

    /// Batch submission mode (only used in sequencer mode)
    /// - anvil: Spawn a local Anvil instance for batch submission (default)
    /// - in-memory: Log batches without persisting them
    /// - remote: Submit to a remote L1 (not yet implemented)
    #[arg(long, default_value = "anvil")]
    pub batch_mode: BatchSubmissionMode,

    /// Disable the TUI interface (TUI is enabled by default)
    #[arg(long, default_value = "false")]
    pub no_tui: bool,

    /// Path to checkpoint file for resumption.
    ///
    /// If the file exists, the node will resume from the last checkpointed state.
    /// If the file doesn't exist, a new checkpoint file will be created.
    #[arg(long, default_value = "./data/checkpoint.json")]
    pub checkpoint_path: PathBuf,

    /// Block producer mode (live or historical)
    #[command(subcommand)]
    pub producer: ProducerMode,
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum ProducerMode {
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
