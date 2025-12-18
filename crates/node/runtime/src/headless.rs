use eyre::Result;
use montana_cli::MontanaCli;
use montana_harness::Harness;

use crate::builder::build_node;

/// Run the node in headless mode (without TUI).
pub async fn run_headless(cli: MontanaCli) -> Result<()> {
    // Spawn harness if enabled (no progress reporter in headless mode)
    let (harness, rpc_url) = Harness::spawn_if_enabled(
        cli.with_harness,
        cli.harness_block_time_ms,
        cli.harness_initial_blocks,
        cli.rpc_url.clone(),
        None, // No TUI progress reporter in headless mode
    )
    .await?;

    let mut node = build_node(cli, None, None, rpc_url).await?;
    let result = node.run().await;
    // Drop harness AFTER node.run() completes to keep anvil alive
    drop(harness);
    result
}
