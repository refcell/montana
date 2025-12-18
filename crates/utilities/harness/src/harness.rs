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

use crate::{BoxedProgressReporter, HarnessConfig};

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
    ///     1000,  // 1 second block time
    ///     10,    // 10 initial blocks
    ///     None,
    ///     None,  // No progress reporter
    /// ).await?;
    /// assert!(harness.is_some());
    /// // Use rpc_url to connect...
    ///
    /// // With harness disabled:
    /// let (harness, rpc_url) = Harness::spawn_if_enabled(
    ///     false,
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
        rpc_url: Option<String>,
        progress: Option<BoxedProgressReporter>,
    ) -> Result<(Option<Self>, String)> {
        if with_harness {
            let config = HarnessConfig {
                block_time_ms,
                initial_delay_blocks: initial_blocks,
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

        // Calculate block time in seconds (minimum 1 second for anvil)
        let block_time_secs = (config.block_time_ms / 1000).max(1);

        // Spawn anvil with configured block time
        let anvil = alloy::node_bindings::Anvil::new()
            .block_time(block_time_secs)
            .try_spawn()
            .map_err(|e| eyre::eyre!("Failed to spawn anvil: {}", e))?;

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
/// Sends `tx_per_block` transactions spread across each block period.
async fn run_tx_generator(
    rpc_url: String,
    signers: Vec<PrivateKeySigner>,
    tx_per_block: u64,
    block_time_ms: u64,
    stop_signal: Arc<AtomicBool>,
) -> Result<()> {
    let addresses: Vec<Address> = signers.iter().map(|s| s.address()).collect();
    let mut rng = rand::thread_rng();

    // Create providers for each signer
    let mut providers = Vec::new();
    for signer in &signers {
        let wallet = EthereumWallet::from(signer.clone());
        let provider = ProviderBuilder::new().wallet(wallet).connect(&rpc_url).await?;
        providers.push(provider);
    }

    // Calculate interval between transactions to spread them across the block time
    // Use 80% of block time to leave buffer before next block
    let effective_block_time_ms = (block_time_ms * 80) / 100;
    let tx_interval_ms = if tx_per_block > 0 {
        effective_block_time_ms / tx_per_block
    } else {
        effective_block_time_ms
    };

    tracing::info!(
        tx_per_block = tx_per_block,
        block_time_ms = block_time_ms,
        tx_interval_ms = tx_interval_ms,
        num_accounts = providers.len(),
        "Transaction generator started - will run indefinitely"
    );

    let mut tx_count: u64 = 0;
    let mut consecutive_errors: u32 = 0;
    const MAX_CONSECUTIVE_ERRORS: u32 = 10;

    loop {
        if stop_signal.load(Ordering::SeqCst) {
            tracing::info!(total_transactions = tx_count, "Transaction generator stopping");
            break;
        }

        // Pick random sender and recipient
        let sender_idx = rng.gen_range(0..providers.len());
        let recipient_idx = (sender_idx + rng.gen_range(1..addresses.len())) % addresses.len();

        let to = addresses[recipient_idx];
        let value = U256::from(rng.gen_range(1_000_000_000_000u64..10_000_000_000_000_000u64));

        let tx = alloy::rpc::types::TransactionRequest::default().to(to).value(value);

        // Send transaction (don't wait for receipt to keep it fast)
        match providers[sender_idx].send_transaction(tx).await {
            Ok(pending) => {
                tx_count += 1;
                consecutive_errors = 0;

                // Log every 100 transactions at debug level, every 10 at trace level
                if tx_count.is_multiple_of(100) {
                    tracing::debug!(
                        tx_count = tx_count,
                        hash = %pending.tx_hash(),
                        "Transaction generator progress"
                    );
                } else {
                    tracing::trace!(
                        from = %addresses[sender_idx],
                        to = %to,
                        value = %value,
                        hash = %pending.tx_hash(),
                        "Sent transaction"
                    );
                }
            }
            Err(e) => {
                consecutive_errors += 1;
                tracing::warn!(
                    error = %e,
                    consecutive_errors = consecutive_errors,
                    "Failed to send transaction"
                );

                // If we get too many consecutive errors, back off more aggressively
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    tracing::error!(
                        consecutive_errors = consecutive_errors,
                        "Too many consecutive transaction errors, backing off for 5 seconds"
                    );
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    consecutive_errors = 0;
                }
            }
        }

        // If tx_interval_ms is 0 (very high tx_per_block), yield to allow other tasks
        if tx_interval_ms > 0 {
            tokio::time::sleep(Duration::from_millis(tx_interval_ms)).await;
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
