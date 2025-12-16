//! Operation mode selection for CLI.

use clap::ValueEnum;

/// Available operation modes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    /// Batch submission mode (default) - compress and submit L2 blocks.
    #[default]
    Batch,
    /// Derivation mode - decompress and derive L2 blocks from compressed batches.
    Derivation,
    /// Roundtrip mode - batch submission followed by derivation with validation.
    Roundtrip,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Batch => write!(f, "batch"),
            Self::Derivation => write!(f, "derivation"),
            Self::Roundtrip => write!(f, "roundtrip"),
        }
    }
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
