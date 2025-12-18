//! Integration tests for the Montana harness.
//!
//! These tests verify that:
//! 1. Anvil spawns correctly and produces blocks
//! 2. The transaction generator runs and submits transactions
//! 3. Blocks are produced continuously over time
//! 4. The harness shuts down gracefully
//!
//! Note: Tests that require anvil are marked with `#[ignore]` and can be run with:
//! ```sh
//! cargo test -p montana-harness --test integration -- --ignored
//! ```

use std::time::Duration;

use alloy::providers::{Provider, ProviderBuilder};
use montana_harness::{Harness, HarnessConfig};

/// Test that the harness spawns anvil and it's accessible via RPC.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_spawns_anvil() {
    let config = HarnessConfig {
        block_time_ms: 1000,
        tx_per_block: 10,
        initial_delay_blocks: 0,
        accounts: 10,
    };

    let harness = Harness::spawn(config).await.expect("Failed to spawn harness");
    let rpc_url = harness.rpc_url();

    // Verify we can connect to anvil
    let provider =
        ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil RPC");

    // Get chain ID to verify anvil is responding
    let chain_id = provider.get_chain_id().await.expect("Failed to get chain ID");
    assert!(chain_id > 0, "Chain ID should be positive");
}

/// Test that initial blocks are generated.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_generates_initial_blocks() {
    let initial_blocks = 5;
    let config = HarnessConfig {
        block_time_ms: 1000,
        tx_per_block: 10,
        initial_delay_blocks: initial_blocks,
        accounts: 10,
    };

    let harness = Harness::spawn(config).await.expect("Failed to spawn harness");
    let rpc_url = harness.rpc_url();

    let provider =
        ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil RPC");

    // Get current block number
    let block_number = provider.get_block_number().await.expect("Failed to get block number");

    // Should have at least initial_blocks (block numbers are 0-indexed, so block 5 means 6 blocks exist)
    assert!(
        block_number >= initial_blocks,
        "Expected at least {} blocks, got {}",
        initial_blocks,
        block_number
    );
}

/// Test that blocks are produced continuously over time.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_produces_blocks_over_time() {
    let config = HarnessConfig {
        block_time_ms: 1000, // 1 second block time
        tx_per_block: 10,    // 10 transactions per block
        initial_delay_blocks: 0,
        accounts: 10,
    };

    let harness = Harness::spawn(config).await.expect("Failed to spawn harness");
    let rpc_url = harness.rpc_url();

    let provider =
        ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil RPC");

    // Get initial block number
    let initial_block = provider.get_block_number().await.expect("Failed to get block number");

    // Wait for 3 seconds (should see ~3 new blocks with 1s block time)
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Get new block number
    let new_block = provider.get_block_number().await.expect("Failed to get block number");

    // Should have produced at least 2 new blocks (accounting for timing variance)
    let blocks_produced = new_block - initial_block;
    assert!(
        blocks_produced >= 2,
        "Expected at least 2 new blocks in 3 seconds, got {}",
        blocks_produced
    );
}

/// Test that transactions are included in blocks.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_generates_transactions() {
    let config = HarnessConfig {
        block_time_ms: 1000, // 1 second block time
        tx_per_block: 10,    // 10 transactions per block
        initial_delay_blocks: 0,
        accounts: 10,
    };

    let harness = Harness::spawn(config).await.expect("Failed to spawn harness");
    let rpc_url = harness.rpc_url();

    let provider =
        ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil RPC");

    // Wait for a few blocks to be produced with transactions
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Get the latest block
    let block = provider
        .get_block_by_number(alloy::eips::BlockNumberOrTag::Latest)
        .full()
        .await
        .expect("Failed to get block")
        .expect("Block should exist");

    // The harness should have generated some transactions
    // Note: We check across multiple blocks since tx timing is random
    let mut total_txs = 0;
    let current_block = provider.get_block_number().await.unwrap();

    for block_num in 1..=current_block.min(5) {
        if let Ok(Some(block)) = provider.get_block_by_number(block_num.into()).full().await {
            total_txs += block.transactions.len();
        }
    }

    // With 100ms interval, over 3 seconds we should have ~30 transactions
    // But due to timing, we just check we have some
    assert!(total_txs > 0, "Expected some transactions across blocks, got {}", total_txs);

    // Also verify the block has a valid structure
    assert!(block.header.gas_limit > 0, "Block should have gas limit");
}

/// Test that harness works with fast block times.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_fast_block_time() {
    let config = HarnessConfig {
        block_time_ms: 500, // 500ms block time (min 1s for anvil, but config should handle)
        tx_per_block: 20,
        initial_delay_blocks: 2,
        accounts: 5,
    };

    let harness = Harness::spawn(config).await.expect("Failed to spawn harness");
    let rpc_url = harness.rpc_url();

    let provider =
        ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil RPC");

    // Should have initial blocks
    let block_number = provider.get_block_number().await.expect("Failed to get block number");
    assert!(block_number >= 2, "Expected at least 2 initial blocks");

    // Wait and verify more blocks are produced
    tokio::time::sleep(Duration::from_secs(2)).await;

    let new_block_number = provider.get_block_number().await.expect("Failed to get block number");
    assert!(new_block_number > block_number, "Expected new blocks to be produced");
}

/// Test that the harness can be dropped without panicking.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_graceful_shutdown() {
    let config = HarnessConfig {
        block_time_ms: 1000,
        tx_per_block: 10,
        initial_delay_blocks: 1,
        accounts: 5,
    };

    let harness = Harness::spawn(config).await.expect("Failed to spawn harness");
    let rpc_url = harness.rpc_url().to_string();

    // Verify anvil is running
    let provider =
        ProviderBuilder::new().connect(&rpc_url).await.expect("Failed to connect to anvil RPC");
    let _ = provider.get_block_number().await.expect("Should be able to query");

    // Drop the harness
    drop(harness);

    // Give it a moment to shut down
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Trying to connect should fail (anvil should be stopped)
    // Note: This may or may not fail depending on how fast anvil shuts down
    // The important thing is that drop() doesn't panic
}

/// Test transaction generator with high frequency.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_high_frequency_transactions() {
    let config = HarnessConfig {
        block_time_ms: 1000,
        tx_per_block: 100, // 100 tx/block
        initial_delay_blocks: 0,
        accounts: 10,
    };

    let harness = Harness::spawn(config).await.expect("Failed to spawn harness");
    let rpc_url = harness.rpc_url();

    let provider =
        ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil RPC");

    // Wait for transactions to accumulate
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check pending transaction count or recent blocks
    let block = provider
        .get_block_by_number(alloy::eips::BlockNumberOrTag::Latest)
        .full()
        .await
        .expect("Failed to get block");

    // Block should exist
    assert!(block.is_some(), "Latest block should exist");
}

/// Test that blocks contain the expected transactions.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_block_contents() {
    let config = HarnessConfig {
        block_time_ms: 1000,
        tx_per_block: 5,
        initial_delay_blocks: 3,
        accounts: 10,
    };

    let harness = Harness::spawn(config).await.expect("Failed to spawn harness");
    let rpc_url = harness.rpc_url();

    let provider =
        ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil RPC");

    // Check that at least one of the initial blocks has transactions
    // Block 1 might be empty if anvil mines before transactions are submitted
    let mut found_tx = false;
    for block_num in 1..=3 {
        if let Some(block) = provider
            .get_block_by_number(block_num.into())
            .full()
            .await
            .expect("Failed to get block")
        {
            // Verify block header fields
            assert!(block.header.number >= 1);
            assert!(block.header.timestamp > 0);

            if !block.transactions.is_empty() {
                found_tx = true;
                break;
            }
        }
    }
    assert!(found_tx, "At least one initial block should have transactions");
}

/// Test `spawn_if_enabled` with harness enabled.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_spawn_if_enabled_with_harness() {
    let (harness, rpc_url) = Harness::spawn_if_enabled(true, 1000, 2, None, None)
        .await
        .expect("Failed to spawn harness");

    // Should return Some(harness)
    assert!(harness.is_some(), "Harness should be spawned when enabled");

    // Should return harness RPC URL
    assert!(rpc_url.starts_with("http://"), "RPC URL should be a valid HTTP URL");

    // Verify we can connect
    let provider =
        ProviderBuilder::new().connect(&rpc_url).await.expect("Failed to connect to harness");

    let block_number = provider.get_block_number().await.expect("Failed to get block number");
    assert!(block_number >= 2, "Should have at least 2 initial blocks");
}

/// Test `spawn_if_enabled` with harness disabled.
#[tokio::test]
async fn test_spawn_if_enabled_without_harness() {
    let custom_rpc = "http://localhost:8545".to_string();
    let (harness, rpc_url) =
        Harness::spawn_if_enabled(false, 1000, 0, Some(custom_rpc.clone()), None)
            .await
            .expect("Should succeed with RPC URL provided");

    // Should return None (no harness)
    assert!(harness.is_none(), "Harness should not be spawned when disabled");

    // Should return the provided RPC URL
    assert_eq!(rpc_url, custom_rpc, "Should return the provided RPC URL");
}

/// Test `spawn_if_enabled` errors when harness disabled but no RPC URL.
#[tokio::test]
async fn test_spawn_if_enabled_error_without_rpc() {
    let result = Harness::spawn_if_enabled(false, 1000, 0, None, None).await;

    // Should return an error
    assert!(result.is_err(), "Should error when harness disabled and no RPC URL provided");

    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("RPC URL is required"),
        "Error should mention RPC URL requirement"
    );
}
