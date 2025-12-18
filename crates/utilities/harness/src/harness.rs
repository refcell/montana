//! Test harness implementation.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use alloy::{
    network::EthereumWallet,
    node_bindings::AnvilInstance,
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
};
use eyre::Result;
use rand::Rng;

use crate::{BoxedProgressReporter, HarnessConfig, config::DEFAULT_GENESIS_STATE};

/// Test harness that manages an anvil instance with synthetic transaction activity.
///
/// The harness:
/// 1. Spawns a local anvil chain
/// 2. Generates initial blocks for sync testing
/// 3. Continuously generates transactions in a background thread
///
/// When dropped, the transaction generator stops and anvil is terminated.
pub struct Harness {
    /// The running anvil instance.
    #[allow(dead_code)]
    anvil: AnvilInstance,
    /// RPC URL for the anvil instance.
    rpc_url: String,
    /// Handle to the background transaction generator thread.
    #[allow(dead_code)]
    tx_handle: Option<JoinHandle<()>>,
    /// Signal to stop the transaction generator.
    stop_signal: Arc<AtomicBool>,
}

impl Harness {
    /// Spawns a harness if enabled by CLI flags, returning the harness and RPC URL.
    ///
    /// If harness is not enabled, returns None and the original RPC URL.
    ///
    /// This is a convenience method for handling the common pattern of conditionally
    /// spawning a test harness based on CLI flags.
    ///
    /// # Arguments
    ///
    /// * `with_harness` - Whether to spawn a harness
    /// * `block_time_ms` - Anvil block time in milliseconds
    /// * `initial_blocks` - Number of blocks to generate before returning
    /// * `rpc_url` - RPC URL to use if harness is not enabled
    /// * `progress` - Optional progress reporter for TUI feedback during initialization
    ///
    /// # Returns
    ///
    /// Returns a tuple of (Option<Harness>, RPC URL):
    /// - If `with_harness` is true: (Some(harness), harness RPC URL)
    /// - If `with_harness` is false: (None, provided RPC URL)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Harness is enabled but fails to spawn
    /// - Harness is not enabled but no RPC URL is provided
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use montana_harness::Harness;
    ///
    /// # async fn example() -> eyre::Result<()> {
    /// // With harness enabled:
    /// let (harness, rpc_url) = Harness::spawn_if_enabled(
    ///     true,
    ///     1000,   // 1 second block time
    ///     10,     // 10 initial blocks
    ///     10_000, // 10k transactions per block
    ///     None,
    ///     None,   // No progress reporter
    /// ).await?;
    /// assert!(harness.is_some());
    /// // Use rpc_url to connect...
    ///
    /// // With harness disabled:
    /// let (harness, rpc_url) = Harness::spawn_if_enabled(
    ///     false,
    ///     0,
    ///     0,
    ///     0,
    ///     Some("http://localhost:8545".to_string()),
    ///     None,
    /// ).await?;
    /// assert!(harness.is_none());
    /// // Use provided rpc_url...
    /// # Ok(())
    /// # }
    /// ```
    pub async fn spawn_if_enabled(
        with_harness: bool,
        block_time_ms: u64,
        initial_blocks: u64,
        tx_per_block: u64,
        rpc_url: Option<String>,
        progress: Option<BoxedProgressReporter>,
    ) -> Result<(Option<Self>, String)> {
        if with_harness {
            let config = HarnessConfig {
                block_time_ms,
                initial_delay_blocks: initial_blocks,
                tx_per_block,
                ..Default::default()
            };
            let harness = Self::spawn_with_progress(config, progress).await?;
            let rpc_url = harness.rpc_url().to_string();
            Ok((Some(harness), rpc_url))
        } else {
            let rpc_url = rpc_url
                .ok_or_else(|| eyre::eyre!("RPC URL is required when harness is disabled"))?;
            Ok((None, rpc_url))
        }
    }

    /// Spawn a new test harness with the given configuration.
    ///
    /// This will:
    /// 1. Start a local anvil instance
    /// 2. Generate `initial_delay_blocks` blocks (if > 0)
    /// 3. Start background transaction generation
    ///
    /// # Errors
    ///
    /// Returns an error if anvil fails to spawn or initial block generation fails.
    pub async fn spawn(config: HarnessConfig) -> Result<Self> {
        Self::spawn_with_progress(config, None).await
    }

    /// Spawn a new test harness with the given configuration and progress reporter.
    ///
    /// This will:
    /// 1. Start a local anvil instance
    /// 2. Generate `initial_delay_blocks` blocks (if > 0), reporting progress
    /// 3. Start background transaction generation
    ///
    /// # Arguments
    ///
    /// * `config` - Harness configuration
    /// * `progress` - Optional progress reporter for TUI feedback during initialization
    ///
    /// # Errors
    ///
    /// Returns an error if anvil fails to spawn or initial block generation fails.
    pub async fn spawn_with_progress(
        config: HarnessConfig,
        progress: Option<BoxedProgressReporter>,
    ) -> Result<Self> {
        tracing::info!(?config, "Spawning test harness");

        let total_blocks = config.initial_delay_blocks;

        // Report that we're starting
        if let Some(ref p) = progress {
            p.report_started(total_blocks);
        }

        // Calculate block time in seconds (supports sub-second via f64)
        let block_time_secs = config.block_time_ms as f64 / 1000.0;

        // Build anvil with configured block time and gas limit
        // block_time_f64 supports sub-second intervals
        // gas_limit sets the max gas per block (default 30M is too low for high throughput)
        let mut anvil_builder = alloy::node_bindings::Anvil::new()
            .block_time_f64(block_time_secs)
            .arg("--gas-limit")
            .arg(config.block_gas_limit.to_string());

        // Track temp file for default genesis (must live until after anvil spawns)
        let _genesis_temp_file: Option<tempfile::NamedTempFile>;

        // Configure state loading/dumping
        // Priority order:
        // 1. If state_path is set with dump_initial_state, dump to that path
        // 2. If state_path is set (without dump), load from that path if it exists
        // 3. If use_default_genesis is true, load the embedded default genesis
        if let Some(ref state_path) = config.state_path {
            let path_str = state_path.to_string_lossy();
            _genesis_temp_file = None;

            if config.dump_initial_state {
                // Only dump state on exit (don't load existing state)
                tracing::info!(path = %path_str, "Will dump Anvil state on exit");
                anvil_builder = anvil_builder.arg("--dump-state").arg(state_path.as_os_str());
            } else if state_path.exists() {
                // Load existing state (don't dump on exit to preserve the original)
                tracing::info!(path = %path_str, "Loading Anvil state from file");
                anvil_builder = anvil_builder.arg("--load-state").arg(state_path.as_os_str());
            } else {
                tracing::info!(path = %path_str, "State file does not exist, starting fresh");
            }
        } else if config.use_default_genesis {
            // Use the embedded default genesis state
            // Write it to a temp file since --load-state requires a file path
            let temp_file = tempfile::Builder::new()
                .prefix("anvil_genesis_")
                .suffix(".json")
                .tempfile()
                .map_err(|e| eyre::eyre!("Failed to create temp file for genesis state: {}", e))?;

            std::fs::write(temp_file.path(), DEFAULT_GENESIS_STATE)
                .map_err(|e| eyre::eyre!("Failed to write default genesis state: {}", e))?;

            tracing::info!(
                path = %temp_file.path().display(),
                "Loading default genesis state (10 accounts with 10000 ETH each)"
            );
            anvil_builder = anvil_builder.arg("--load-state").arg(temp_file.path());

            // Keep the temp file alive until anvil reads it
            _genesis_temp_file = Some(temp_file);
        } else {
            _genesis_temp_file = None;
            tracing::info!("Starting Anvil fresh (no genesis state)");
        }

        let anvil =
            anvil_builder.try_spawn().map_err(|e| eyre::eyre!("Failed to spawn anvil: {}", e))?;

        let rpc_url = anvil.endpoint();
        tracing::info!(rpc_url = %rpc_url, "Anvil started");

        // Report anvil started
        if let Some(ref p) = progress {
            p.report_progress(0, total_blocks, &format!("Anvil started at {}", rpc_url));
        }

        // Get signers from anvil's pre-funded accounts
        let signers: Vec<PrivateKeySigner> =
            anvil.keys().iter().take(config.accounts as usize).map(|k| k.clone().into()).collect();

        let addresses: Vec<Address> = signers.iter().map(|s| s.address()).collect();
        tracing::info!(accounts = addresses.len(), "Using test accounts");

        // Create provider with first signer for initial block generation
        let wallet = EthereumWallet::from(signers[0].clone());
        let provider = ProviderBuilder::new().wallet(wallet).connect(&rpc_url).await?;

        // Set a faster poll interval for sub-second block times
        // Default is 250ms for local transports, but we need faster polling to match our block time
        // Use the configured block time as the poll interval (minimum 10ms to avoid spinning)
        let poll_interval = Duration::from_millis(config.block_time_ms.max(10));
        provider.client().set_poll_interval(poll_interval);
        tracing::debug!(?poll_interval, "Set provider poll interval for fast block times");

        // Generate initial blocks if configured
        if total_blocks > 0 {
            tracing::info!(blocks = total_blocks, "Generating initial blocks for sync testing");

            if let Some(ref p) = progress {
                p.report_progress(0, total_blocks, "Generating initial blocks...");
            }

            for i in 0..total_blocks {
                // Send a simple transfer to trigger a block
                let to = addresses[(i as usize + 1) % addresses.len()];
                let value = U256::from(1_000_000_000_000_000u64); // 0.001 ETH

                let tx = alloy::rpc::types::TransactionRequest::default().to(to).value(value);

                provider.send_transaction(tx).await?.watch().await?;

                let block_num = i + 1;

                // Report progress after each block
                if let Some(ref p) = progress {
                    p.report_progress(
                        block_num,
                        total_blocks,
                        &format!("Generated block #{}", block_num),
                    );
                }

                if block_num % 10 == 0 {
                    tracing::debug!(block = block_num, "Generated initial block");
                }
            }

            tracing::info!(blocks = total_blocks, "Initial block generation complete");

            if let Some(ref p) = progress {
                p.report_completed(total_blocks);
            }
        }

        // Set up stop signal for background thread
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_signal_clone = Arc::clone(&stop_signal);

        // Spawn background transaction generator
        let tx_per_block = config.tx_per_block;
        let block_time_ms = config.block_time_ms;
        let rpc_url_clone = rpc_url.clone();
        let signers_clone = signers.clone();

        let tx_handle = std::thread::spawn(move || {
            // Create a dedicated runtime for the tx generator
            let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to create tx generator runtime");
                    return;
                }
            };

            rt.block_on(async move {
                if let Err(e) = run_tx_generator(
                    rpc_url_clone,
                    signers_clone,
                    tx_per_block,
                    block_time_ms,
                    stop_signal_clone,
                )
                .await
                {
                    tracing::error!(error = %e, "Transaction generator error");
                }
            });
        });

        Ok(Self { anvil, rpc_url, tx_handle: Some(tx_handle), stop_signal })
    }

    /// Get the RPC URL for the anvil instance.
    ///
    /// Use this URL to connect Montana to the test harness.
    #[must_use]
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        // Signal the tx generator to stop
        self.stop_signal.store(true, Ordering::SeqCst);
        tracing::info!("Harness shutting down");
    }
}

impl std::fmt::Debug for Harness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Harness").field("rpc_url", &self.rpc_url).finish()
    }
}

/// Background transaction generator.
///
/// Continuously sends random transfers between test accounts until stopped.
/// Runs indefinitely to simulate a real chain that never stops producing blocks.
/// Sends transactions in parallel batches for maximum throughput.
async fn run_tx_generator(
    rpc_url: String,
    signers: Vec<PrivateKeySigner>,
    tx_per_block: u64,
    block_time_ms: u64,
    stop_signal: Arc<AtomicBool>,
) -> Result<()> {
    use futures::stream::{FuturesUnordered, StreamExt};

    let addresses: Vec<Address> = signers.iter().map(|s| s.address()).collect();

    // Create providers for each signer
    let mut providers = Vec::new();
    for signer in &signers {
        let wallet = EthereumWallet::from(signer.clone());
        let provider = ProviderBuilder::new().wallet(wallet).connect(&rpc_url).await?;
        providers.push(Arc::new(provider));
    }
    let providers = Arc::new(providers);
    let addresses = Arc::new(addresses);

    // Calculate batch size and interval
    // Send tx_per_block transactions per block_time_ms
    // Use parallel batches - limited to avoid exhausting file descriptors
    // 10 parallel transactions × 10 accounts = manageable connection count
    let batch_size = 10usize;
    let batches_per_block = (tx_per_block as usize).div_ceil(batch_size);
    let batch_interval_ms = if batches_per_block > 0 {
        (block_time_ms as usize * 80 / 100) / batches_per_block
    } else {
        block_time_ms as usize
    };

    tracing::info!(
        tx_per_block = tx_per_block,
        block_time_ms = block_time_ms,
        batch_size = batch_size,
        batches_per_block = batches_per_block,
        batch_interval_ms = batch_interval_ms,
        num_accounts = providers.len(),
        "Transaction generator started - will run indefinitely with parallel batches"
    );

    let tx_count = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut consecutive_errors: u32 = 0;
    const MAX_CONSECUTIVE_ERRORS: u32 = 50;

    loop {
        if stop_signal.load(Ordering::SeqCst) {
            tracing::info!(
                total_transactions = tx_count.load(Ordering::Relaxed),
                "Transaction generator stopping"
            );
            break;
        }

        // Send a batch of transactions in parallel
        let mut futures = FuturesUnordered::new();

        for i in 0..batch_size {
            let providers = Arc::clone(&providers);
            let addresses = Arc::clone(&addresses);
            let tx_count = Arc::clone(&tx_count);

            futures.push(async move {
                let mut rng = rand::thread_rng();
                let sender_idx = i % providers.len();
                let recipient_idx =
                    (sender_idx + rng.gen_range(1..addresses.len())) % addresses.len();

                let to = addresses[recipient_idx];
                let value =
                    U256::from(rng.gen_range(1_000_000_000_000u64..10_000_000_000_000_000u64));

                let tx = alloy::rpc::types::TransactionRequest::default().to(to).value(value);

                match providers[sender_idx].send_transaction(tx).await {
                    Ok(pending) => {
                        let count = tx_count.fetch_add(1, Ordering::Relaxed) + 1;
                        if count.is_multiple_of(1000) {
                            tracing::debug!(
                                tx_count = count,
                                hash = %pending.tx_hash(),
                                "Transaction generator progress"
                            );
                        }
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            });
        }

        // Wait for all transactions in the batch to complete
        let mut batch_errors = 0u32;
        while let Some(result) = futures.next().await {
            if result.is_err() {
                batch_errors += 1;
            }
        }

        if batch_errors > 0 {
            consecutive_errors += batch_errors;
            if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                tracing::warn!(
                    consecutive_errors = consecutive_errors,
                    "Many transaction errors, backing off for 1 second"
                );
                tokio::time::sleep(Duration::from_secs(1)).await;
                consecutive_errors = 0;
            }
        } else {
            consecutive_errors = 0;
        }

        // Sleep between batches if needed
        if batch_interval_ms > 0 {
            tokio::time::sleep(Duration::from_millis(batch_interval_ms as u64)).await;
        } else {
            tokio::task::yield_now().await;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harness_debug() {
        // Just test that Debug is implemented correctly
        let debug = format!("{:?}", HarnessConfig::default());
        assert!(debug.contains("HarnessConfig"));
    }
}
