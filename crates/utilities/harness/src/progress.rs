//! Progress reporting for harness initialization.
//!
//! This module provides a trait for reporting progress during initial block
//! generation, allowing the TUI to show feedback instead of appearing empty.

/// Trait for reporting harness initialization progress.
///
/// Implement this trait to receive progress updates during initial block
/// generation. This is typically implemented by a TUI handle adapter.
pub trait HarnessProgressReporter: Send + Sync {
    /// Report progress during initial block generation.
    ///
    /// # Arguments
    ///
    /// * `current_block` - The block number just generated (1-indexed)
    /// * `total_blocks` - Total number of blocks to generate
    /// * `message` - A human-readable status message
    fn report_progress(&self, current_block: u64, total_blocks: u64, message: &str);

    /// Report that the harness has started initializing.
    ///
    /// Called before the first block is generated.
    fn report_started(&self, total_blocks: u64) {
        self.report_progress(0, total_blocks, "Starting anvil...");
    }

    /// Report that initialization is complete.
    ///
    /// Called after all initial blocks have been generated.
    /// The final_block parameter indicates the last block generated during init.
    fn report_completed(&self, total_blocks: u64) {
        self.report_progress(total_blocks, total_blocks, "Initialization complete");
        // Signal that init is done - implementations should override to send HarnessInitComplete
        self.report_init_complete(total_blocks);
    }

    /// Signal that initialization is complete and no more init blocks will be logged.
    ///
    /// This is called after `report_completed` and should trigger the
    /// `HarnessInitComplete` event to prevent re-logging of init blocks.
    ///
    /// # Arguments
    ///
    /// * `final_block` - The last block number generated during initialization
    fn report_init_complete(&self, _final_block: u64) {
        // Default implementation does nothing - override to send event
    }
}

/// A boxed progress reporter for dynamic dispatch.
pub type BoxedProgressReporter = Box<dyn HarnessProgressReporter>;
