//! Transaction send state machine.

use std::collections::HashSet;

use alloy::primitives::B256;

/// Tracks the state of a single transaction submission attempt.
#[derive(Debug, Default)]
pub struct SendState {
    /// Transaction hashes that have been mined (may reorg)
    mined_txs: HashSet<B256>,
    /// Number of successful publishes to mempool
    successful_publish_count: u32,
    /// Count of "nonce too low" errors
    nonce_too_low_count: u32,
    /// Transaction type conflict detected
    #[allow(dead_code)]
    already_reserved: bool,
    /// Flag to bump fees on next attempt
    bump_fees: bool,
    /// Total number of fee bumps performed
    bump_count: u32,
}

impl SendState {
    /// Creates a new send state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mined_txs: HashSet::new(),
            successful_publish_count: 0,
            nonce_too_low_count: 0,
            already_reserved: false,
            bump_fees: false,
            bump_count: 0,
        }
    }

    /// Records a successful transaction publish to the mempool.
    pub const fn tx_published(&mut self) {
        self.successful_publish_count += 1;
    }

    /// Records that a transaction hash has been mined.
    pub fn tx_mined(&mut self, hash: B256) {
        self.mined_txs.insert(hash);
    }

    /// Records that a transaction hash was not mined or has been reorged.
    ///
    /// If no transactions remain mined, resets the nonce too low count.
    pub fn tx_not_mined(&mut self, hash: &B256) {
        self.mined_txs.remove(hash);
        if self.mined_txs.is_empty() {
            self.nonce_too_low_count = 0;
        }
    }

    /// Records a "nonce too low" error.
    pub const fn nonce_too_low(&mut self) {
        self.nonce_too_low_count += 1;
    }

    /// Returns whether the nonce should be aborted based on the threshold.
    ///
    /// # Arguments
    ///
    /// * `threshold` - Maximum number of "nonce too low" errors to tolerate (0 = never abort)
    #[must_use]
    pub const fn should_abort_nonce(&self, threshold: u32) -> bool {
        threshold > 0 && self.nonce_too_low_count >= threshold
    }

    /// Returns whether the transaction was successfully published at least once.
    #[must_use]
    pub const fn was_published(&self) -> bool {
        self.successful_publish_count > 0
    }

    /// Sets the bump fees flag and increments the bump count.
    pub const fn set_bump_fees(&mut self) {
        self.bump_fees = true;
        self.bump_count += 1;
    }

    /// Takes and clears the bump fees flag.
    ///
    /// Returns the previous value of the flag.
    pub const fn take_bump_fees(&mut self) -> bool {
        let prev = self.bump_fees;
        self.bump_fees = false;
        prev
    }

    /// Returns the total number of fee bumps performed.
    #[must_use]
    pub const fn bump_count(&self) -> u32 {
        self.bump_count
    }

    /// Returns the number of mined transactions.
    #[must_use]
    pub fn mined_count(&self) -> usize {
        self.mined_txs.len()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn send_state_new() {
        let state = SendState::new();
        assert_eq!(state.successful_publish_count, 0);
        assert_eq!(state.nonce_too_low_count, 0);
        assert!(!state.already_reserved);
        assert!(!state.bump_fees);
        assert_eq!(state.bump_count, 0);
        assert_eq!(state.mined_count(), 0);
    }

    #[test]
    fn send_state_default() {
        let state = SendState::default();
        assert_eq!(state.successful_publish_count, 0);
        assert_eq!(state.nonce_too_low_count, 0);
        assert!(!state.already_reserved);
        assert!(!state.bump_fees);
        assert_eq!(state.bump_count, 0);
        assert_eq!(state.mined_count(), 0);
    }

    #[test]
    fn tx_published_increments_count() {
        let mut state = SendState::new();
        assert_eq!(state.successful_publish_count, 0);
        assert!(!state.was_published());

        state.tx_published();
        assert_eq!(state.successful_publish_count, 1);
        assert!(state.was_published());

        state.tx_published();
        assert_eq!(state.successful_publish_count, 2);
        assert!(state.was_published());
    }

    #[test]
    fn tx_mined_adds_to_set() {
        let mut state = SendState::new();
        let hash1 = B256::from([1u8; 32]);
        let hash2 = B256::from([2u8; 32]);

        assert_eq!(state.mined_count(), 0);

        state.tx_mined(hash1);
        assert_eq!(state.mined_count(), 1);

        state.tx_mined(hash2);
        assert_eq!(state.mined_count(), 2);

        // Adding same hash again should not increase count
        state.tx_mined(hash1);
        assert_eq!(state.mined_count(), 2);
    }

    #[test]
    fn tx_not_mined_removes_from_set() {
        let mut state = SendState::new();
        let hash1 = B256::from([1u8; 32]);
        let hash2 = B256::from([2u8; 32]);

        state.tx_mined(hash1);
        state.tx_mined(hash2);
        assert_eq!(state.mined_count(), 2);

        state.tx_not_mined(&hash1);
        assert_eq!(state.mined_count(), 1);

        state.tx_not_mined(&hash2);
        assert_eq!(state.mined_count(), 0);

        // Removing non-existent hash should not fail
        state.tx_not_mined(&hash1);
        assert_eq!(state.mined_count(), 0);
    }

    #[test]
    fn tx_not_mined_resets_nonce_too_low_when_empty() {
        let mut state = SendState::new();
        let hash1 = B256::from([1u8; 32]);
        let hash2 = B256::from([2u8; 32]);

        state.tx_mined(hash1);
        state.tx_mined(hash2);
        state.nonce_too_low();
        state.nonce_too_low();
        assert_eq!(state.nonce_too_low_count, 2);

        // Remove one hash - nonce_too_low_count should remain
        state.tx_not_mined(&hash1);
        assert_eq!(state.nonce_too_low_count, 2);

        // Remove last hash - nonce_too_low_count should reset
        state.tx_not_mined(&hash2);
        assert_eq!(state.nonce_too_low_count, 0);
    }

    #[test]
    fn nonce_too_low_increments_count() {
        let mut state = SendState::new();
        assert_eq!(state.nonce_too_low_count, 0);

        state.nonce_too_low();
        assert_eq!(state.nonce_too_low_count, 1);

        state.nonce_too_low();
        assert_eq!(state.nonce_too_low_count, 2);
    }

    #[rstest]
    #[case(0, 0, false)]
    #[case(0, 1, false)]
    #[case(1, 1, true)]
    #[case(2, 1, true)]
    #[case(5, 10, false)]
    #[case(10, 10, true)]
    #[case(11, 10, true)]
    fn should_abort_nonce_with_threshold(
        #[case] nonce_too_low_count: u32,
        #[case] threshold: u32,
        #[case] expected: bool,
    ) {
        let mut state = SendState::new();
        for _ in 0..nonce_too_low_count {
            state.nonce_too_low();
        }
        assert_eq!(state.should_abort_nonce(threshold), expected);
    }

    #[rstest]
    #[case(0, false)]
    #[case(1, true)]
    #[case(2, true)]
    #[case(10, true)]
    fn was_published_based_on_publish_count(#[case] publish_count: u32, #[case] expected: bool) {
        let mut state = SendState::new();
        for _ in 0..publish_count {
            state.tx_published();
        }
        assert_eq!(state.was_published(), expected);
    }

    #[test]
    fn set_bump_fees_sets_flag_and_increments_count() {
        let mut state = SendState::new();
        assert!(!state.bump_fees);
        assert_eq!(state.bump_count(), 0);

        state.set_bump_fees();
        assert!(state.bump_fees);
        assert_eq!(state.bump_count(), 1);

        state.set_bump_fees();
        assert!(state.bump_fees);
        assert_eq!(state.bump_count(), 2);
    }

    #[test]
    fn take_bump_fees_returns_and_clears_flag() {
        let mut state = SendState::new();
        assert!(!state.bump_fees);

        // Take when false
        assert!(!state.take_bump_fees());
        assert!(!state.bump_fees);

        // Set and take
        state.set_bump_fees();
        assert!(state.bump_fees);
        assert!(state.take_bump_fees());
        assert!(!state.bump_fees);

        // Take again - should be false
        assert!(!state.take_bump_fees());
        assert!(!state.bump_fees);
    }

    #[test]
    fn bump_count_persists_after_take_bump_fees() {
        let mut state = SendState::new();

        state.set_bump_fees();
        assert_eq!(state.bump_count(), 1);

        state.take_bump_fees();
        assert_eq!(state.bump_count(), 1);

        state.set_bump_fees();
        assert_eq!(state.bump_count(), 2);

        state.take_bump_fees();
        assert_eq!(state.bump_count(), 2);
    }

    #[test]
    fn send_state_debug() {
        let state = SendState::new();
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("SendState"));
    }

    #[test]
    fn complex_state_transitions() {
        let mut state = SendState::new();
        let hash1 = B256::from([1u8; 32]);
        let hash2 = B256::from([2u8; 32]);

        // Initial publish
        state.tx_published();
        assert!(state.was_published());
        assert_eq!(state.successful_publish_count, 1);

        // Mine first tx
        state.tx_mined(hash1);
        assert_eq!(state.mined_count(), 1);

        // Get nonce too low errors
        state.nonce_too_low();
        state.nonce_too_low();
        assert!(!state.should_abort_nonce(3));
        assert!(state.should_abort_nonce(2));

        // Set bump fees
        state.set_bump_fees();
        assert_eq!(state.bump_count(), 1);
        assert!(state.take_bump_fees());

        // Publish again with bumped fees
        state.tx_published();
        assert_eq!(state.successful_publish_count, 2);

        // Mine second tx
        state.tx_mined(hash2);
        assert_eq!(state.mined_count(), 2);

        // Reorg first tx
        state.tx_not_mined(&hash1);
        assert_eq!(state.mined_count(), 1);
        assert_eq!(state.nonce_too_low_count, 2); // Should not reset yet

        // Reorg second tx
        state.tx_not_mined(&hash2);
        assert_eq!(state.mined_count(), 0);
        assert_eq!(state.nonce_too_low_count, 0); // Should reset now
    }

    #[test]
    fn multiple_fee_bumps_tracked() {
        let mut state = SendState::new();

        for i in 1..=5 {
            state.set_bump_fees();
            assert_eq!(state.bump_count(), i);
            assert!(state.bump_fees);

            let taken = state.take_bump_fees();
            assert!(taken);
            assert!(!state.bump_fees);
            assert_eq!(state.bump_count(), i);
        }
    }

    #[test]
    fn mined_count_with_duplicate_hashes() {
        let mut state = SendState::new();
        let hash = B256::from([42u8; 32]);

        state.tx_mined(hash);
        assert_eq!(state.mined_count(), 1);

        // Adding the same hash multiple times
        state.tx_mined(hash);
        state.tx_mined(hash);
        assert_eq!(state.mined_count(), 1);

        state.tx_not_mined(&hash);
        assert_eq!(state.mined_count(), 0);
    }
}
