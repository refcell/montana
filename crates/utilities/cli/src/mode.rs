//! Operation mode selection for CLI.

use clap::ValueEnum;

/// Available operation modes.
///
/// Defines the three operation modes supported by Montana for working with L2 blocks
/// and compressed batches.
///
/// # Examples
///
/// ```
/// use montana_cli::Mode;
///
/// // Get the default mode
/// let default = Mode::default();
/// assert_eq!(default, Mode::Batch);
///
/// // Display mode as string
/// assert_eq!(Mode::Derivation.to_string(), "derivation");
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum, derive_more::Display)]
pub enum Mode {
    /// Batch submission mode (default) - compress and submit L2 blocks.
    ///
    /// Takes L2 block data from the input file, applies the selected compression algorithm,
    /// and writes the compressed batch to the output file.
    #[default]
    #[display("batch")]
    Batch,
    /// Derivation mode - decompress and derive L2 blocks from compressed batches.
    ///
    /// Takes compressed batch data from the input file, decompresses it using the selected
    /// algorithm, and writes the derived L2 blocks to the output file.
    #[display("derivation")]
    Derivation,
    /// Roundtrip mode - batch submission followed by derivation with validation.
    ///
    /// Performs batch submission followed by derivation on the result, then validates
    /// that the roundtrip process produces identical output to the input.
    #[display("roundtrip")]
    Roundtrip,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_default() {
        assert_eq!(Mode::default(), Mode::Batch);
    }

    #[test]
    fn mode_display() {
        assert_eq!(Mode::Batch.to_string(), "batch");
        assert_eq!(Mode::Derivation.to_string(), "derivation");
        assert_eq!(Mode::Roundtrip.to_string(), "roundtrip");
    }

    #[test]
    fn mode_clone() {
        let mode = Mode::Batch;
        let cloned = mode;
        assert_eq!(mode, cloned);
    }

    #[test]
    fn mode_debug() {
        let debug = format!("{:?}", Mode::Batch);
        assert!(debug.contains("Batch"));
    }
}
