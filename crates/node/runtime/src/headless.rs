use std::sync::{Arc, atomic::AtomicBool};

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
        cli.harness_tx_per_block,
        cli.rpc_url.clone(),
        None, // No TUI progress reporter in headless mode
    )
    .await?;

    // In headless mode, use CLI flag for blob mode
    let use_blobs = Some(Arc::new(AtomicBool::new(cli.use_blobs)));
    let mut node = build_node(cli, None, None, rpc_url, use_blobs).await?;
    let result = node.run().await;
    // Drop harness AFTER node.run() completes to keep anvil alive
    drop(harness);
    result
}
