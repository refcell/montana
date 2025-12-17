#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

/// CLI argument parsing and command execution.
///
/// The [`Cli`] struct provides the main interface for parsing command-line arguments
/// using the `clap` crate. It includes options for verbosity, operation mode, input/output
/// file paths, and compression algorithm selection.
mod cli;
pub use cli::Cli;

/// Compression algorithm selection.
///
/// The [`CompressionAlgorithm`] enum provides support for multiple compression algorithms
/// (Brotli, Zlib, Zstd) and includes a utility method to iterate over all single algorithms.
mod compression;
pub use compression::CompressionAlgorithm;

/// Operation mode selection.
///
/// The [`Mode`] enum defines the available operation modes: batch submission, derivation,
/// and roundtrip validation.
mod mode;
pub use mode::Mode;

/// Montana binary operating mode.
///
/// The [`MontanaMode`] enum defines how the Montana binary operates: as a pure executor,
/// a full sequencer, or a validator.
mod montana_mode;
pub use montana_mode::MontanaMode;

/// Montana CLI argument parsing.
///
/// The [`MontanaCli`] struct provides the main interface for parsing command-line arguments
/// for the Montana node binary. It includes options for RPC URLs, operating mode, headless
/// mode, sync behavior, checkpointing, logging, and batch submission.
mod montana_cli;
pub use montana_cli::MontanaCli;
// Re-export BatchSubmissionMode from montana_batcher
pub use montana_batcher::BatchSubmissionMode;

/// Tracing initialization utilities.
///
/// The [`init_tracing`] function configures the tracing subscriber with a verbosity-based
/// log level and respects the `RUST_LOG` environment variable.
mod tracing;
pub use tracing::init_tracing;
