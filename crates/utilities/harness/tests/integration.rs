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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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
        ..Default::default()
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

/// Test that anvil produces blocks at approximately the correct sub-second rate.
///
/// This test verifies that `block_time_f64` is correctly configured with anvil
/// by measuring actual block production timing.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_50ms_block_time() {
    use std::time::Instant;

    let config = HarnessConfig {
        block_time_ms: 50, // 50ms block time (20 blocks per second)
        tx_per_block: 10,
        initial_delay_blocks: 0, // No initial blocks - we want to measure production rate
        accounts: 5,
        ..Default::default()
    };

    let harness = Harness::spawn(config).await.expect("Failed to spawn harness");
    let rpc_url = harness.rpc_url();

    let provider =
        ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil RPC");

    // Wait a moment for anvil to stabilize
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Get initial block number
    let initial_block = provider.get_block_number().await.expect("Failed to get block number");

    // Measure how many blocks are produced in 2 seconds
    // With 50ms block time, we expect ~40 blocks in 2 seconds
    let start = Instant::now();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let elapsed = start.elapsed();

    let final_block = provider.get_block_number().await.expect("Failed to get block number");
    let blocks_produced = final_block.saturating_sub(initial_block);

    // Calculate actual blocks per second
    let blocks_per_second = blocks_produced as f64 / elapsed.as_secs_f64();

    // With 50ms block time, we expect 20 blocks/sec
    // Allow a tolerance of ±30% to account for timing variance
    let expected_blocks_per_second = 1000.0 / 50.0; // 20 blocks/sec
    let min_expected = expected_blocks_per_second * 0.7; // 14 blocks/sec
    let max_expected = expected_blocks_per_second * 1.3; // 26 blocks/sec

    println!(
        "Block production rate: {:.2} blocks/sec (expected: {:.2}, range: {:.2}-{:.2})",
        blocks_per_second, expected_blocks_per_second, min_expected, max_expected
    );
    println!(
        "Produced {} blocks in {:.2}s (initial: {}, final: {})",
        blocks_produced,
        elapsed.as_secs_f64(),
        initial_block,
        final_block
    );

    assert!(
        blocks_per_second >= min_expected,
        "Block production too slow: {:.2} blocks/sec, expected at least {:.2}. \
         Produced {} blocks in {:.2}s. This suggests block_time_f64 may not be working correctly.",
        blocks_per_second,
        min_expected,
        blocks_produced,
        elapsed.as_secs_f64()
    );

    assert!(
        blocks_per_second <= max_expected,
        "Block production too fast: {:.2} blocks/sec, expected at most {:.2}",
        blocks_per_second,
        max_expected
    );
}

/// Test that initial block generation with 50ms block time works correctly.
///
/// This test verifies that the harness can generate initial blocks quickly
/// when using sub-second block times.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_50ms_initial_blocks() {
    use std::time::Instant;

    let initial_blocks = 10;
    let block_time_ms = 50;

    let config = HarnessConfig {
        block_time_ms,
        tx_per_block: 10,
        initial_delay_blocks: initial_blocks,
        accounts: 5,
        ..Default::default()
    };

    // Measure how long it takes to spawn (which includes initial block generation)
    let start = Instant::now();
    let harness = Harness::spawn(config).await.expect("Failed to spawn harness");
    let spawn_time = start.elapsed();

    let rpc_url = harness.rpc_url();
    let provider =
        ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil RPC");

    // Verify we have the initial blocks
    let block_number = provider.get_block_number().await.expect("Failed to get block number");
    assert!(
        block_number >= initial_blocks,
        "Expected at least {} initial blocks, got {}",
        initial_blocks,
        block_number
    );

    // With 50ms block time and 10 blocks, should take ~500ms for block generation
    // Plus setup overhead, let's say max 3 seconds is reasonable
    let max_expected_time = Duration::from_secs(3);

    println!(
        "Harness spawn with {} initial blocks ({}ms block time) took {:?}",
        initial_blocks, block_time_ms, spawn_time
    );
    println!("Block number after spawn: {} (expected >= {})", block_number, initial_blocks);

    assert!(
        spawn_time < max_expected_time,
        "Harness spawn took too long: {:?}, expected < {:?}. \
         This suggests block_time_f64 may not be working correctly for initial block generation.",
        spawn_time,
        max_expected_time
    );
}

/// Test that initial state can be dumped to a file.
///
/// This verifies the `dump_initial_state` configuration works correctly
/// and produces a valid state file when Anvil exits.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_dumps_initial_state() {
    use tempfile::tempdir;

    // Create a temporary directory for the state file
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let state_path = temp_dir.path().join("anvil_state.json");

    let config = HarnessConfig {
        block_time_ms: 1000,
        tx_per_block: 0, // No transactions to get clean genesis state
        initial_delay_blocks: 0, // No initial blocks - we want clean genesis state
        accounts: 10,
        use_default_genesis: false, // Don't use default - we're testing dump functionality
        state_path: Some(state_path.clone()),
        dump_initial_state: true,
    };

    // Spawn harness and immediately drop it to trigger state dump on exit
    {
        let harness = Harness::spawn(config).await.expect("Failed to spawn harness");
        let rpc_url = harness.rpc_url();

        // Verify anvil is running
        let provider =
            ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil");
        let _ = provider.get_chain_id().await.expect("Failed to get chain ID");

        // Drop harness - this triggers the --dump-state on exit
        drop(harness);
    }

    // Wait for anvil to fully shut down and write state
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify state file was created
    assert!(state_path.exists(), "State file should have been created on exit");

    // Verify it has content
    let state_data = std::fs::read(&state_path).expect("Failed to read state file");
    assert!(!state_data.is_empty(), "State file should not be empty");

    println!("State file created with {} bytes", state_data.len());
}

/// Test that state can be loaded from a previously dumped file.
///
/// This verifies the full cycle: dump state, then load it in a new instance.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_loads_state_from_file() {
    use tempfile::tempdir;

    // Create a temporary directory for the state file
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let state_path = temp_dir.path().join("anvil_state.json");

    // Step 1: Spawn harness and dump initial state (state is dumped on exit)
    {
        let config = HarnessConfig {
            block_time_ms: 1000,
            tx_per_block: 0, // No transactions
            initial_delay_blocks: 0, // No initial blocks for clean genesis
            accounts: 10,
            use_default_genesis: false, // Testing dump functionality
            state_path: Some(state_path.clone()),
            dump_initial_state: true,
        };

        let harness = Harness::spawn(config).await.expect("Failed to spawn harness for dump");
        let rpc_url = harness.rpc_url();

        // Get the chain ID and genesis block hash to compare later
        let provider =
            ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil");

        let chain_id = provider.get_chain_id().await.expect("Failed to get chain ID");
        let block_0 = provider
            .get_block_by_number(0.into())
            .full()
            .await
            .expect("Failed to get block 0")
            .expect("Block 0 should exist");

        println!(
            "Original instance - Chain ID: {}, Block 0 hash: {:?}",
            chain_id, block_0.header.hash
        );

        // Drop harness to stop anvil and trigger state dump
        drop(harness);
    }

    // Wait for anvil to fully shut down and write state file
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify state file was created
    assert!(state_path.exists(), "State file should exist after first instance shutdown");

    // Step 2: Spawn new harness loading from the state file
    {
        let config = HarnessConfig {
            block_time_ms: 1000,
            tx_per_block: 0,
            initial_delay_blocks: 0,
            accounts: 10,
            use_default_genesis: false, // Using custom state file
            state_path: Some(state_path.clone()),
            dump_initial_state: false, // Load state, don't overwrite
        };

        let harness = Harness::spawn(config).await.expect("Failed to spawn harness with loaded state");
        let rpc_url = harness.rpc_url();

        let provider =
            ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil");

        // Verify we can query the chain
        let chain_id = provider.get_chain_id().await.expect("Failed to get chain ID");
        assert!(chain_id > 0, "Chain ID should be positive");

        let block_number = provider.get_block_number().await.expect("Failed to get block number");
        println!(
            "Loaded instance - Chain ID: {}, Block number: {}",
            chain_id, block_number
        );

        // The instance should have loaded the state successfully
        // Block number should be 0 since we dumped at genesis with no initial blocks
        assert_eq!(
            block_number, 0,
            "Block number should be 0 when loading fresh genesis state"
        );
    }
}

/// Test that loaded state preserves account balances.
///
/// This verifies that the pre-funded test accounts are correctly preserved
/// when loading state from a file.
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_state_preserves_accounts() {
    use alloy::primitives::utils::format_ether;
    use tempfile::tempdir;

    // Create a temporary directory for the state file
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let state_path = temp_dir.path().join("anvil_state.json");

    // The expected balance for Anvil's default test accounts
    // Anvil gives each account 10000 ETH by default
    let expected_balance_eth = 10000.0;

    // Step 1: Spawn harness and record account addresses/balances
    let (addresses, original_balances) = {
        let config = HarnessConfig {
            block_time_ms: 1000,
            tx_per_block: 0, // No transactions to preserve exact balances
            initial_delay_blocks: 0,
            accounts: 10,
            use_default_genesis: false, // Testing state file functionality
            state_path: Some(state_path.clone()),
            dump_initial_state: true,
        };

        let harness = Harness::spawn(config).await.expect("Failed to spawn harness");
        let rpc_url = harness.rpc_url();

        let provider =
            ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil");

        // Get all accounts (Anvil's deterministic test accounts)
        let accounts: Vec<alloy::primitives::Address> =
            provider.get_accounts().await.expect("Failed to get accounts");

        // Get balances for all accounts
        let mut balances = Vec::new();
        for account in &accounts {
            let balance = provider.get_balance(*account).await.expect("Failed to get balance");
            balances.push(balance);

            let balance_eth: f64 = format_ether(balance).parse().unwrap();
            println!("Account {}: {} ETH", account, balance_eth);

            // Verify original balance is approximately 10000 ETH
            assert!(
                (balance_eth - expected_balance_eth).abs() < 1.0,
                "Original balance should be ~10000 ETH, got {} ETH",
                balance_eth
            );
        }

        (accounts, balances)
    };

    // Wait for anvil to fully shut down
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Step 2: Load state and verify balances are preserved
    {
        let config = HarnessConfig {
            block_time_ms: 1000,
            tx_per_block: 0,
            initial_delay_blocks: 0,
            accounts: 10,
            use_default_genesis: false, // Using custom state file
            state_path: Some(state_path.clone()),
            dump_initial_state: false,
        };

        let harness = Harness::spawn(config).await.expect("Failed to spawn harness with loaded state");
        let rpc_url = harness.rpc_url();

        let provider =
            ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil");

        // Get the accounts again (should be same deterministic accounts)
        let loaded_accounts: Vec<alloy::primitives::Address> =
            provider.get_accounts().await.expect("Failed to get accounts");

        // Verify same accounts
        assert_eq!(
            addresses.len(),
            loaded_accounts.len(),
            "Should have same number of accounts"
        );

        for (i, (original, loaded)) in addresses.iter().zip(loaded_accounts.iter()).enumerate() {
            assert_eq!(
                original, loaded,
                "Account {} should match: {:?} vs {:?}",
                i, original, loaded
            );
        }

        // Verify balances are preserved
        for (i, (account, original_balance)) in
            loaded_accounts.iter().zip(original_balances.iter()).enumerate()
        {
            let loaded_balance = provider.get_balance(*account).await.expect("Failed to get balance");

            assert_eq!(
                *original_balance, loaded_balance,
                "Account {} balance should be preserved: {:?} vs {:?}",
                i, original_balance, loaded_balance
            );

            let balance_eth: f64 = format_ether(loaded_balance).parse().unwrap();
            println!("Loaded account {}: {} ETH (preserved)", account, balance_eth);
        }

        println!("All {} account balances preserved correctly!", loaded_accounts.len());
    }
}

/// Test that the default embedded genesis state works correctly.
///
/// This verifies that:
/// 1. The default genesis loads without errors
/// 2. All 10 pre-funded accounts are available
/// 3. Each account has exactly 10000 ETH
#[ignore = "requires anvil"]
#[tokio::test]
async fn test_harness_default_genesis() {
    use alloy::primitives::utils::format_ether;

    // Use default config which has use_default_genesis: true
    let config = HarnessConfig::default();

    let harness = Harness::spawn(config).await.expect("Failed to spawn harness with default genesis");
    let rpc_url = harness.rpc_url();

    let provider =
        ProviderBuilder::new().connect(rpc_url).await.expect("Failed to connect to anvil");

    // Verify chain is running
    let chain_id = provider.get_chain_id().await.expect("Failed to get chain ID");
    assert_eq!(chain_id, 31337, "Chain ID should be Anvil's default 31337");

    // Get all accounts
    let accounts: Vec<alloy::primitives::Address> =
        provider.get_accounts().await.expect("Failed to get accounts");

    // Should have 10 pre-funded accounts
    assert_eq!(accounts.len(), 10, "Should have 10 pre-funded accounts");

    // Verify each account has approximately 10000 ETH
    // Some accounts may have spent a tiny amount on gas during initial block generation
    let expected_balance_eth = 10000.0;
    for (i, account) in accounts.iter().enumerate() {
        let balance = provider.get_balance(*account).await.expect("Failed to get balance");
        let balance_eth: f64 = format_ether(balance).parse().unwrap();

        println!("Account {}: {} - {} ETH", i, account, balance_eth);

        // Allow 1 ETH tolerance for gas spent during initial block generation
        assert!(
            (balance_eth - expected_balance_eth).abs() < 1.0,
            "Account {} should have ~10000 ETH, got {} ETH",
            i,
            balance_eth
        );
    }

    println!("Default genesis loaded successfully with all 10 accounts at 10000 ETH each");
}
