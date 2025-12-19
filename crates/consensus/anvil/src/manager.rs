//! Anvil instance manager.

use std::sync::{Arc, atomic::AtomicBool};

use alloy::{
    network::EthereumWallet,
    node_bindings::AnvilInstance,
    primitives::Address,
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
};

use crate::{AnvilBatchSink, AnvilBatchSource, AnvilError};

/// Configuration for an Anvil instance.
#[derive(Debug, Clone)]
pub struct AnvilConfig {
    /// Batch inbox address (where batches are sent).
    pub batch_inbox: Address,
    /// Block time in seconds (0 for auto-mine).
    pub block_time: u64,
}

impl Default for AnvilConfig {
    fn default() -> Self {
        Self {
            // Default batch inbox address
            batch_inbox: Address::repeat_byte(0x42),
            // Auto-mine (instant blocks)
            block_time: 0,
        }
    }
}

/// A boxed provider trait object for use with Anvil.
pub(crate) type BoxedProvider = Box<dyn Provider + Send + Sync>;

/// Manages the lifecycle of an Anvil instance.
///
/// When this struct is dropped, the Anvil process is terminated.
///
/// # Example
///
/// ```ignore
/// use montana_anvil::{AnvilConfig, AnvilManager};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let manager = AnvilManager::spawn(AnvilConfig::default()).await?;
///
///     println!("Anvil endpoint: {}", manager.endpoint());
///     println!("Sender address: {:?}", manager.sender());
///
///     // Use manager.sink() and manager.source() for batch operations
///     Ok(())
/// }
/// ```
pub struct AnvilManager {
    /// The running Anvil instance.
    #[allow(dead_code)]
    anvil: AnvilInstance,
    /// Provider for the Anvil instance.
    provider: Arc<BoxedProvider>,
    /// Sender address.
    sender: Address,
    /// Configuration.
    config: AnvilConfig,
    /// The endpoint URL.
    endpoint_url: String,
}

impl AnvilManager {
    /// Spawn a new Anvil instance with the given configuration.
    ///
    /// This will start an Anvil process and wait for it to be ready.
    ///
    /// # Errors
    ///
    /// Returns an error if Anvil fails to spawn or the provider fails to connect.
    pub async fn spawn(config: AnvilConfig) -> Result<Self, AnvilError> {
        // Build Anvil instance
        let mut anvil_builder = alloy::node_bindings::Anvil::new();
        if config.block_time > 0 {
            anvil_builder = anvil_builder.block_time(config.block_time);
        }

        let anvil = anvil_builder.try_spawn().map_err(|e| AnvilError::Spawn(e.to_string()))?;

        // Get the first default signer
        let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
        let sender = signer.address();
        let wallet = EthereumWallet::from(signer);

        let endpoint_url = anvil.endpoint();

        // Build provider with wallet
        let provider = ProviderBuilder::new()
            .wallet(wallet)
            .connect(&endpoint_url)
            .await
            .map_err(|e| AnvilError::Connection(e.to_string()))?;

        // Box the provider to use as trait object
        let boxed_provider: BoxedProvider = Box::new(provider);

        Ok(Self { anvil, provider: Arc::new(boxed_provider), sender, config, endpoint_url })
    }

    /// Get the RPC endpoint URL.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint_url
    }

    /// Get the sender address.
    #[must_use]
    pub const fn sender(&self) -> Address {
        self.sender
    }

    /// Get the batch inbox address.
    #[must_use]
    pub const fn batch_inbox(&self) -> Address {
        self.config.batch_inbox
    }

    /// Create a sink for submitting batches.
    ///
    /// # Arguments
    ///
    /// * `use_blobs` - Shared atomic flag controlling blob vs calldata mode.
    ///   When true, batches are submitted as EIP-4844 blob transactions.
    ///   When false, batches are submitted as calldata transactions.
    #[must_use]
    pub fn sink(&self, use_blobs: Arc<AtomicBool>) -> AnvilBatchSink {
        AnvilBatchSink::new(
            Arc::clone(&self.provider),
            self.sender,
            self.config.batch_inbox,
            use_blobs,
        )
    }

    /// Create a sink for submitting batches with default blob mode.
    ///
    /// This creates a sink that defaults to using blobs (EIP-4844).
    /// For dynamic control, use [`sink`] instead.
    #[must_use]
    pub fn sink_default(&self) -> AnvilBatchSink {
        self.sink(Arc::new(AtomicBool::new(true)))
    }

    /// Create a source for reading batches.
    #[must_use]
    pub fn source(&self) -> AnvilBatchSource {
        AnvilBatchSource::new(Arc::clone(&self.provider), self.config.batch_inbox)
    }
}

impl std::fmt::Debug for AnvilManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnvilManager")
            .field("endpoint", &self.endpoint_url)
            .field("sender", &self.sender)
            .field("batch_inbox", &self.config.batch_inbox)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anvil_config_default() {
        let config = AnvilConfig::default();
        assert_eq!(config.batch_inbox, Address::repeat_byte(0x42));
        assert_eq!(config.block_time, 0);
    }

    #[test]
    fn anvil_config_custom() {
        let config = AnvilConfig { batch_inbox: Address::repeat_byte(0x01), block_time: 12 };
        assert_eq!(config.batch_inbox, Address::repeat_byte(0x01));
        assert_eq!(config.block_time, 12);
    }
}
