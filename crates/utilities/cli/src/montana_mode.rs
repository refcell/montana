//! Operating mode selection for the Montana binary.

use clap::ValueEnum;

/// Operating mode for the Montana binary.
///
/// Defines how the Montana binary operates - as a pure executor, a full sequencer,
/// a validator, or dual mode running both sequencer and validator.
///
/// # Examples
///
/// ```
/// use montana_cli::MontanaMode;
///
/// // Get the default mode
/// let default = MontanaMode::default();
/// assert_eq!(default, MontanaMode::Dual);
///
/// // Display mode as string
/// assert_eq!(MontanaMode::Dual.to_string(), "dual");
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum, derive_more::Display)]
pub enum MontanaMode {
    /// Executor mode - execute blocks and verify against RPC.
    ///
    /// Fetches L2 blocks, executes them locally, and verifies the results
    /// against RPC receipts. Does not submit batches to L1.
    #[display("executor")]
    Executor,
    /// Sequencer mode - execute blocks and submit batches to L1.
    ///
    /// Runs as a full sequencer: executes L2 blocks and submits compressed
    /// batches to L1 via the batch submission pipeline.
    #[display("sequencer")]
    Sequencer,
    /// Validator mode - derive and validate blocks from L1.
    ///
    /// Reads compressed batches from L1, derives L2 blocks, and validates
    /// the execution results.
    #[display("validator")]
    Validator,
    /// Dual mode (default) - run both sequencer and validator concurrently.
    ///
    /// Executes L2 blocks, submits batches to L1, then derives and validates
    /// blocks from L1.
    #[default]
    #[display("dual")]
    Dual,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn montana_mode_default() {
        assert_eq!(MontanaMode::default(), MontanaMode::Dual);
    }

    #[test]
    fn montana_mode_display() {
        assert_eq!(MontanaMode::Executor.to_string(), "executor");
        assert_eq!(MontanaMode::Sequencer.to_string(), "sequencer");
        assert_eq!(MontanaMode::Validator.to_string(), "validator");
        assert_eq!(MontanaMode::Dual.to_string(), "dual");
    }

    #[test]
    fn montana_mode_clone() {
        let mode = MontanaMode::Sequencer;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    #[test]
    fn montana_mode_debug() {
        let debug = format!("{:?}", MontanaMode::Executor);
        assert!(debug.contains("Executor"));
    }

    #[test]
    fn montana_mode_equality() {
        assert_eq!(MontanaMode::Executor, MontanaMode::Executor);
        assert_ne!(MontanaMode::Executor, MontanaMode::Sequencer);
        assert_ne!(MontanaMode::Sequencer, MontanaMode::Validator);
        assert_ne!(MontanaMode::Dual, MontanaMode::Validator);
    }
}
