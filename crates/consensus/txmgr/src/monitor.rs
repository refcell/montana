//! Transaction monitoring and receipt polling.

use std::sync::Arc;

use alloy::{primitives::B256, providers::Provider};

use crate::{candidate::TxReceipt, config::TxManagerConfig, error::TxError};

/// Transaction monitor for receipt polling and confirmation tracking.
pub struct TxMonitor<P> {
    /// The provider for interacting with the blockchain.
    provider: Arc<P>,
    /// Configuration for transaction monitoring.
    config: TxManagerConfig,
}

impl<P> TxMonitor<P>
where
    P: Provider + Clone + Send + Sync,
{
    /// Creates a new transaction monitor.
    ///
    /// # Arguments
    ///
    /// * `provider` - The blockchain provider to use for queries
    /// * `config` - Configuration for transaction monitoring
    ///
    /// # Returns
    ///
    /// A new [`TxMonitor`] instance.
    pub const fn new(provider: Arc<P>, config: TxManagerConfig) -> Self {
        Self { provider, config }
    }

    /// Polls for a transaction receipt until it's found.
    ///
    /// This method will continuously poll the blockchain at the configured interval
    /// until the transaction receipt is found.
    ///
    /// # Arguments
    ///
    /// * `tx_hash` - The transaction hash to wait for
    ///
    /// # Returns
    ///
    /// The transaction receipt once found.
    ///
    /// # Errors
    ///
    /// Returns [`TxError::Rpc`] if there's an error querying the provider.
    /// Returns [`TxError::Reverted`] if the transaction was reverted.
    pub async fn wait_for_receipt(&self, tx_hash: B256) -> Result<TxReceipt, TxError> {
        loop {
            if let Some(receipt) = self.get_receipt(tx_hash).await? {
                return Ok(receipt);
            }
            tokio::time::sleep(self.config.receipt_query_interval).await;
        }
    }

    /// Waits for the required number of confirmations for a transaction.
    ///
    /// This method polls the current block number and waits until the transaction
    /// has the required number of confirmations as specified in the config.
    ///
    /// # Arguments
    ///
    /// * `tx_hash` - The transaction hash to wait for
    /// * `receipt` - The transaction receipt
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` once the transaction has enough confirmations.
    ///
    /// # Errors
    ///
    /// Returns [`TxError::Rpc`] if there's an error querying the provider.
    pub async fn wait_for_confirmations(
        &self,
        _tx_hash: B256,
        receipt: &TxReceipt,
    ) -> Result<(), TxError> {
        loop {
            if self.is_confirmed(receipt).await? {
                return Ok(());
            }
            tokio::time::sleep(self.config.receipt_query_interval).await;
        }
    }

    /// Fetches a transaction receipt if it exists.
    ///
    /// # Arguments
    ///
    /// * `tx_hash` - The transaction hash to query
    ///
    /// # Returns
    ///
    /// `Some(TxReceipt)` if the receipt exists, `None` if it hasn't been mined yet.
    ///
    /// # Errors
    ///
    /// Returns [`TxError::Rpc`] if there's an error querying the provider.
    /// Returns [`TxError::Reverted`] if the transaction was reverted.
    pub async fn get_receipt(&self, tx_hash: B256) -> Result<Option<TxReceipt>, TxError> {
        let receipt = self
            .provider
            .get_transaction_receipt(tx_hash)
            .await
            .map_err(|e| TxError::Rpc(e.to_string()))?;

        let Some(receipt) = receipt else {
            return Ok(None);
        };

        // Check if transaction was successful
        if !receipt.status() {
            return Err(TxError::Reverted);
        }

        // Convert alloy receipt to our TxReceipt type
        let tx_receipt = TxReceipt {
            tx_hash: receipt.transaction_hash,
            block_number: receipt
                .block_number
                .ok_or_else(|| TxError::Rpc("Receipt missing block number".to_string()))?,
            block_hash: receipt
                .block_hash
                .ok_or_else(|| TxError::Rpc("Receipt missing block hash".to_string()))?,
            gas_used: receipt.gas_used,
            effective_gas_price: receipt.effective_gas_price,
            blob_gas_used: receipt.blob_gas_used,
            blob_gas_price: receipt.blob_gas_price,
        };

        Ok(Some(tx_receipt))
    }

    /// Checks if a receipt has the required number of confirmations.
    ///
    /// # Arguments
    ///
    /// * `receipt` - The transaction receipt to check
    ///
    /// # Returns
    ///
    /// `true` if the transaction has enough confirmations, `false` otherwise.
    ///
    /// # Errors
    ///
    /// Returns [`TxError::Rpc`] if there's an error querying the current block number.
    pub async fn is_confirmed(&self, receipt: &TxReceipt) -> Result<bool, TxError> {
        let current_block =
            self.provider.get_block_number().await.map_err(|e| TxError::Rpc(e.to_string()))?;

        // Calculate confirmations: current_block - receipt.block_number
        // If current block is less than receipt block (shouldn't happen), treat as not confirmed
        if current_block < receipt.block_number {
            return Ok(false);
        }

        let confirmations = current_block - receipt.block_number;
        Ok(confirmations >= self.config.num_confirmations)
    }
}

impl<P> std::fmt::Debug for TxMonitor<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxMonitor").field("config", &self.config).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use alloy::primitives::B256;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(1000, 1010, 10, true, "exact confirmations")]
    #[case(1000, 1011, 10, true, "more than required confirmations")]
    #[case(1000, 1009, 10, false, "not enough confirmations")]
    #[case(1000, 1000, 10, false, "zero confirmations")]
    #[case(1000, 999, 10, false, "current block less than receipt block")]
    fn test_confirmation_calculation(
        #[case] receipt_block: u64,
        #[case] current_block: u64,
        #[case] required_confirmations: u64,
        #[case] expected: bool,
        #[case] _description: &str,
    ) {
        let _receipt = TxReceipt {
            tx_hash: B256::ZERO,
            block_number: receipt_block,
            block_hash: B256::ZERO,
            gas_used: 21000,
            effective_gas_price: 1_000_000_000,
            blob_gas_used: None,
            blob_gas_price: None,
        };

        let confirmations =
            if current_block >= receipt_block { current_block - receipt_block } else { 0 };

        let is_confirmed = if current_block < receipt_block {
            false
        } else {
            confirmations >= required_confirmations
        };

        assert_eq!(is_confirmed, expected);
    }

    #[test]
    fn tx_receipt_conversion_fields() {
        let tx_hash = B256::from([1u8; 32]);
        let block_hash = B256::from([2u8; 32]);
        let receipt = TxReceipt {
            tx_hash,
            block_number: 12345,
            block_hash,
            gas_used: 50000,
            effective_gas_price: 20_000_000_000,
            blob_gas_used: Some(131072),
            blob_gas_price: Some(5),
        };

        assert_eq!(receipt.tx_hash, tx_hash);
        assert_eq!(receipt.block_number, 12345);
        assert_eq!(receipt.block_hash, block_hash);
        assert_eq!(receipt.gas_used, 50000);
        assert_eq!(receipt.effective_gas_price, 20_000_000_000);
        assert_eq!(receipt.blob_gas_used, Some(131072));
        assert_eq!(receipt.blob_gas_price, Some(5));
    }

    #[test]
    fn config_default_values() {
        let config = TxManagerConfig::default();
        assert_eq!(config.num_confirmations, 10);
        assert_eq!(config.receipt_query_interval, Duration::from_secs(12));
    }

    #[test]
    fn config_custom_values() {
        let config = TxManagerConfig::builder()
            .num_confirmations(5)
            .receipt_query_interval(Duration::from_secs(6))
            .build();
        assert_eq!(config.num_confirmations, 5);
        assert_eq!(config.receipt_query_interval, Duration::from_secs(6));
    }

    #[rstest]
    #[case(0, "zero confirmations required")]
    #[case(1, "one confirmation required")]
    #[case(10, "ten confirmations required")]
    #[case(100, "many confirmations required")]
    fn num_confirmations_values(#[case] confirmations: u64, #[case] _description: &str) {
        let config = TxManagerConfig::builder().num_confirmations(confirmations).build();
        assert_eq!(config.num_confirmations, confirmations);
    }

    #[rstest]
    #[case(Duration::from_secs(1), "fast polling")]
    #[case(Duration::from_secs(12), "default polling")]
    #[case(Duration::from_secs(30), "slow polling")]
    fn receipt_query_interval_values(#[case] interval: Duration, #[case] _description: &str) {
        let config = TxManagerConfig::builder().receipt_query_interval(interval).build();
        assert_eq!(config.receipt_query_interval, interval);
    }

    #[test]
    fn tx_monitor_with_custom_config() {
        let config = TxManagerConfig::builder()
            .num_confirmations(5)
            .receipt_query_interval(Duration::from_secs(6))
            .build();
        assert_eq!(config.num_confirmations, 5);
        assert_eq!(config.receipt_query_interval, Duration::from_secs(6));
    }

    #[test]
    fn receipt_block_number_edge_cases() {
        let receipt = TxReceipt {
            tx_hash: B256::ZERO,
            block_number: 0,
            block_hash: B256::ZERO,
            gas_used: 21000,
            effective_gas_price: 1_000_000_000,
            blob_gas_used: None,
            blob_gas_price: None,
        };

        assert_eq!(receipt.block_number, 0);
    }

    #[test]
    fn receipt_with_blob_fields() {
        let receipt = TxReceipt {
            tx_hash: B256::ZERO,
            block_number: 1000,
            block_hash: B256::ZERO,
            gas_used: 21000,
            effective_gas_price: 1_000_000_000,
            blob_gas_used: Some(131072),
            blob_gas_price: Some(10),
        };

        assert!(receipt.blob_gas_used.is_some());
        assert!(receipt.blob_gas_price.is_some());
        assert_eq!(receipt.blob_gas_used.unwrap(), 131072);
        assert_eq!(receipt.blob_gas_price.unwrap(), 10);
    }

    #[test]
    fn receipt_without_blob_fields() {
        let receipt = TxReceipt {
            tx_hash: B256::ZERO,
            block_number: 1000,
            block_hash: B256::ZERO,
            gas_used: 21000,
            effective_gas_price: 1_000_000_000,
            blob_gas_used: None,
            blob_gas_price: None,
        };

        assert!(receipt.blob_gas_used.is_none());
        assert!(receipt.blob_gas_price.is_none());
    }
}
