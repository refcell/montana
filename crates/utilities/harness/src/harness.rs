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

use crate::HarnessConfig;

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
        tracing::info!(?config, "Spawning test harness");

        // Calculate block time in seconds (minimum 1 second for anvil)
        let block_time_secs = (config.block_time_ms / 1000).max(1);

        // Spawn anvil with configured block time
        let anvil = alloy::node_bindings::Anvil::new()
            .block_time(block_time_secs)
            .try_spawn()
            .map_err(|e| eyre::eyre!("Failed to spawn anvil: {}", e))?;

        let rpc_url = anvil.endpoint();
        tracing::info!(rpc_url = %rpc_url, "Anvil started");

        // Get signers from anvil's pre-funded accounts
        let signers: Vec<PrivateKeySigner> =
            anvil.keys().iter().take(config.accounts as usize).map(|k| k.clone().into()).collect();

        let addresses: Vec<Address> = signers.iter().map(|s| s.address()).collect();
        tracing::info!(accounts = addresses.len(), "Using test accounts");

        // Create provider with first signer for initial block generation
        let wallet = EthereumWallet::from(signers[0].clone());
        let provider = ProviderBuilder::new().wallet(wallet).connect(&rpc_url).await?;

        // Generate initial blocks if configured
        if config.initial_delay_blocks > 0 {
            tracing::info!(
                blocks = config.initial_delay_blocks,
                "Generating initial blocks for sync testing"
            );

            for i in 0..config.initial_delay_blocks {
                // Send a simple transfer to trigger a block
                let to = addresses[(i as usize + 1) % addresses.len()];
                let value = U256::from(1_000_000_000_000_000u64); // 0.001 ETH

                let tx = alloy::rpc::types::TransactionRequest::default().to(to).value(value);

                provider.send_transaction(tx).await?.watch().await?;

                if (i + 1) % 10 == 0 {
                    tracing::debug!(block = i + 1, "Generated initial block");
                }
            }

            tracing::info!(
                blocks = config.initial_delay_blocks,
                "Initial block generation complete"
            );
        }

        // Set up stop signal for background thread
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_signal_clone = Arc::clone(&stop_signal);

        // Spawn background transaction generator
        let tx_interval_ms = config.tx_interval_ms;
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
                    tx_interval_ms,
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
async fn run_tx_generator(
    rpc_url: String,
    signers: Vec<PrivateKeySigner>,
    tx_interval_ms: u64,
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

    tracing::info!("Transaction generator started");

    loop {
        if stop_signal.load(Ordering::SeqCst) {
            tracing::info!("Transaction generator stopping");
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
                tracing::trace!(
                    from = %addresses[sender_idx],
                    to = %to,
                    value = %value,
                    hash = %pending.tx_hash(),
                    "Sent transaction"
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to send transaction");
            }
        }

        tokio::time::sleep(Duration::from_millis(tx_interval_ms)).await;
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
