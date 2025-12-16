//! Nonce tracking and management.

use std::{fmt, sync::Arc};

use alloy::{primitives::Address, providers::Provider};
use tokio::sync::RwLock;

use crate::error::TxError;

/// Thread-safe nonce tracker.
///
/// Manages transaction nonces for a specific address, maintaining a local cache
/// that synchronizes with on-chain state when needed. The tracker automatically
/// fetches the nonce from the provider on first use or when explicitly reset.
///
/// # Type Parameters
///
/// * `P` - Provider type that must implement `Provider + Clone + Send + Sync`
///
/// # Examples
///
/// ```ignore
/// use alloy::primitives::Address;
/// use alloy::providers::Provider;
/// use montana_txmgr::NonceTracker;
/// use std::sync::Arc;
///
/// async fn example<P: Provider + Clone + Send + Sync>(
///     provider: Arc<P>,
///     address: Address,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let tracker = NonceTracker::new(provider, address);
///
///     // Get next nonce (fetches from chain if needed)
///     let nonce = tracker.next_nonce().await?;
///
///     // Peek at current nonce without incrementing
///     let current = tracker.current().await;
///
///     // Reset and re-sync with chain
///     tracker.reset().await?;
///
///     Ok(())
/// }
/// ```
pub struct NonceTracker<P> {
    /// The provider used to fetch nonces from the chain.
    provider: Arc<P>,
    /// The address being tracked.
    address: Address,
    /// The cached nonce value. None indicates the nonce needs to be fetched.
    nonce: RwLock<Option<u64>>,
}

impl<P> NonceTracker<P>
where
    P: Provider + Clone + Send + Sync,
{
    /// Creates a new nonce tracker for the given address.
    ///
    /// The nonce is not fetched immediately; it will be fetched on the first call
    /// to [`next_nonce`](Self::next_nonce) or [`reset`](Self::reset).
    ///
    /// # Arguments
    ///
    /// * `provider` - Provider to use for fetching nonces from the chain
    /// * `address` - The address to track nonces for
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use alloy::primitives::Address;
    /// use montana_txmgr::NonceTracker;
    /// use std::sync::Arc;
    ///
    /// let tracker = NonceTracker::new(
    ///     Arc::new(provider),
    ///     "0x1234567890123456789012345678901234567890".parse().unwrap(),
    /// );
    /// ```
    pub fn new(provider: Arc<P>, address: Address) -> Self {
        Self { provider, address, nonce: RwLock::new(None) }
    }

    /// Returns the next nonce and increments the internal counter.
    ///
    /// If the nonce has not been fetched yet (or has been reset), this will
    /// fetch it from the provider first. Subsequent calls will increment the
    /// cached value without hitting the provider.
    ///
    /// # Errors
    ///
    /// Returns [`TxError::Rpc`] if fetching the nonce from the provider fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let nonce1 = tracker.next_nonce().await?; // Fetches from chain: 5
    /// let nonce2 = tracker.next_nonce().await?; // Uses cache: 6
    /// let nonce3 = tracker.next_nonce().await?; // Uses cache: 7
    /// ```
    pub async fn next_nonce(&self) -> Result<u64, TxError> {
        let mut nonce = self.nonce.write().await;

        if nonce.is_none() {
            // Fetch from provider on first use
            let chain_nonce = self
                .provider
                .get_transaction_count(self.address)
                .await
                .map_err(|e| TxError::Rpc(format!("Failed to fetch nonce: {}", e)))?;

            *nonce = Some(chain_nonce);
        }

        let current = nonce.expect("nonce should be Some after initialization");
        *nonce = Some(current + 1);

        Ok(current)
    }

    /// Resets the nonce by syncing with the current chain state.
    ///
    /// This fetches the latest transaction count from the provider and updates
    /// the internal cache. Useful when a transaction fails or when you want to
    /// ensure the nonce is in sync with the chain.
    ///
    /// # Errors
    ///
    /// Returns [`TxError::Rpc`] if fetching the nonce from the provider fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // After a failed transaction, reset to chain state
    /// tracker.reset().await?;
    /// let fresh_nonce = tracker.next_nonce().await?;
    /// ```
    pub async fn reset(&self) -> Result<(), TxError> {
        let chain_nonce = self
            .provider
            .get_transaction_count(self.address)
            .await
            .map_err(|e| TxError::Rpc(format!("Failed to reset nonce: {}", e)))?;

        let mut nonce = self.nonce.write().await;
        *nonce = Some(chain_nonce);

        Ok(())
    }

    /// Returns the current cached nonce without incrementing it.
    ///
    /// Returns `None` if the nonce hasn't been fetched yet. This method does
    /// not fetch from the provider; use [`next_nonce`](Self::next_nonce) or
    /// [`reset`](Self::reset) to fetch the nonce.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let current = tracker.current().await;
    /// assert_eq!(current, None); // Not fetched yet
    ///
    /// let nonce = tracker.next_nonce().await?;
    /// let current = tracker.current().await;
    /// assert_eq!(current, Some(nonce + 1)); // Points to next available nonce
    /// ```
    pub async fn current(&self) -> Option<u64> {
        *self.nonce.read().await
    }

    /// Returns the address being tracked.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let address: Address = "0x1234567890123456789012345678901234567890".parse().unwrap();
    /// let tracker = NonceTracker::new(Arc::new(provider), address);
    /// assert_eq!(tracker.address(), address);
    /// ```
    #[must_use]
    pub const fn address(&self) -> Address {
        self.address
    }
}

impl<P> fmt::Debug for NonceTracker<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NonceTracker")
            .field("address", &self.address)
            .field("nonce", &"<locked>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy::primitives::Address;

    fn test_address() -> Address {
        Address::from_str("0x1234567890123456789012345678901234567890").unwrap()
    }

    #[test]
    fn test_address_parsing() {
        let addr = test_address();
        // Address debug format is 0x + 40 hex chars = 42 chars
        assert_eq!(format!("{:?}", addr).len(), 42);
    }

    #[test]
    fn test_address_from_str() {
        let addr1 = Address::from_str("0x1111111111111111111111111111111111111111").unwrap();
        let addr2 = Address::from_str("0x2222222222222222222222222222222222222222").unwrap();
        assert_ne!(addr1, addr2);
    }

    #[test]
    fn test_address_equality() {
        let addr1 = test_address();
        let addr2 = test_address();
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_zero_address() {
        let zero = Address::ZERO;
        assert_eq!(zero, Address::from_str("0x0000000000000000000000000000000000000000").unwrap());
    }
}
