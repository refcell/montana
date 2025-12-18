#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use clap::Parser;
use eyre::Result;
use montana_cli::{MontanaCli, MontanaMode, init_tracing_with_level, install_ctrlc_handler};
use montana_runtime::{run_headless, run_with_tui};

#[tokio::main]
async fn main() -> Result<()> {
    // Install Ctrl+C handler early so it's active during all operations.
    // This sets a shutdown flag instead of calling process::exit(), allowing
    // graceful cleanup of child processes (like anvil).
    install_ctrlc_handler();

    let cli = MontanaCli::parse();

    // Check for unsupported executor mode
    if cli.mode == MontanaMode::Executor {
        eprintln!("Error: Executor mode is not yet supported in the new node architecture.");
        eprintln!("Please use 'sequencer', 'validator', or 'dual' mode instead.");
        std::process::exit(1);
    }

    // Validate RPC URL or harness mode
    if !cli.with_harness && cli.rpc_url.is_none() {
        eprintln!("Error: --rpc-url is required unless --with-harness is enabled.");
        std::process::exit(1);
    }

    if cli.with_harness && cli.rpc_url.is_some() {
        eprintln!("Warning: --rpc-url is ignored when --with-harness is enabled.");
    }

    // In harness mode, we always use the block feeder (skip_sync) to get continuous
    // TUI updates. The sync stage doesn't emit per-block events, so the harness panel
    // would be empty without this.
    let mut cli = cli;
    if cli.with_harness && !cli.skip_sync {
        cli.skip_sync = true;
        // Start from block 0 to sync through the initial harness blocks
        if cli.start.is_none() {
            cli.start = Some(0);
        }
    }

    // Reset checkpoint if requested or if running with harness (fresh start for demos)
    if (cli.reset_checkpoint || cli.with_harness) && cli.checkpoint_path.exists() {
        tracing::info!("Resetting checkpoint at {:?}", cli.checkpoint_path);
        if let Err(e) = std::fs::remove_file(&cli.checkpoint_path) {
            tracing::warn!("Failed to remove checkpoint file: {}", e);
        }
    }

    if cli.headless {
        init_tracing_with_level(&cli.log_level);
        run_headless(cli).await
    } else {
        run_with_tui(cli)
    }
}
