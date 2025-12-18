use std::{io, sync::Arc};

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use eyre::Result;
use montana_adapters::HarnessProgressAdapter;
use montana_cli::{MontanaCli, MontanaMode};
use montana_harness::Harness;
use montana_tui::{TuiObserver, create_tui};

use crate::builder::build_node;

/// Run the node with TUI interface.
pub fn run_with_tui(cli: MontanaCli) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let (tui, handle) = create_tui();

    // Send mode info to TUI immediately after creation
    let node_role = match cli.mode {
        MontanaMode::Sequencer => "Sequencer",
        MontanaMode::Validator => "Validator",
        MontanaMode::Dual => "Dual",
        MontanaMode::Executor => "Executor",
    }
    .to_string();

    handle.send(montana_tui::TuiEvent::ModeInfo {
        node_role,
        start_block: cli.start,
        skip_sync: cli.skip_sync,
    });

    // Send harness mode event if enabled
    if cli.with_harness {
        handle.send(montana_tui::TuiEvent::HarnessModeEnabled);
    }

    // Clone handle for block feeder before creating observer
    let block_feeder_handle = handle.clone();
    // Clone handle for harness progress reporting
    let harness_progress_handle = handle.clone();
    let tui_observer = Arc::new(TuiObserver::new(handle));

    let rt = tokio::runtime::Runtime::new()?;

    // Use a JoinHandle so we can wait for the thread to finish (and drop harness)
    let node_thread = std::thread::spawn(move || {
        rt.block_on(async move {
            // Create progress reporter for harness initialization
            let progress_reporter = if cli.with_harness {
                Some(Box::new(HarnessProgressAdapter::new(harness_progress_handle))
                    as Box<dyn montana_harness::HarnessProgressReporter>)
            } else {
                None
            };

            // Spawn harness if enabled, with progress reporting to TUI
            let (harness, rpc_url) = match Harness::spawn_if_enabled(
                cli.with_harness,
                cli.harness_block_time_ms,
                cli.harness_initial_blocks,
                cli.rpc_url.clone(),
                progress_reporter,
            )
            .await
            {
                Ok(result) => result,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to spawn harness");
                    return;
                }
            };

            let result =
                match build_node(cli, Some(tui_observer), Some(block_feeder_handle), rpc_url).await
                {
                    Ok(mut node) => node.run().await,
                    Err(e) => Err(e),
                };

            if let Err(e) = result {
                tracing::error!(error = %e, "Node error");
            }

            // Drop harness AFTER node.run() completes to keep anvil alive
            drop(harness);
        });
    });

    let result = tui.run();

    // Clean up terminal BEFORE waiting for node thread
    // (so user sees normal terminal while waiting for cleanup)
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    ratatui::restore();

    // Wait for the node thread to finish, which ensures harness is dropped
    // and anvil is properly killed before we exit
    if let Err(e) = node_thread.join() {
        tracing::error!("Node thread panicked: {:?}", e);
    }

    result.map_err(|e| eyre::eyre!("TUI error: {}", e))
}
