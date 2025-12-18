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

/// Batch submission mode selection.
///
/// The [`BatchSubmissionMode`] enum defines the available batch submission modes: in-memory,
/// anvil, and remote.
mod batch_submission_mode;
pub use batch_submission_mode::BatchSubmissionMode;

/// Montana CLI argument parsing.
///
/// The [`MontanaCli`] struct provides the main interface for parsing command-line arguments
/// for the Montana node binary. It includes options for RPC URLs, operating mode, headless
/// mode, sync behavior, checkpointing, logging, and batch submission.
mod montana_cli;
pub use montana_cli::MontanaCli;

/// Tracing initialization utilities.
///
/// The [`init_tracing`] function configures the tracing subscriber with a verbosity-based
/// log level and respects the `RUST_LOG` environment variable.
/// The [`init_tracing_with_level`] function configures the tracing subscriber with a
/// string-based log level.
mod tracing_init;
pub use crate::tracing_init::{init_tracing, init_tracing_with_level};

/// Ctrl+C signal handler utilities.
///
/// The [`install_ctrlc_handler`] function installs a handler that exits on Ctrl+C.
/// The [`wait_for_ctrlc`] function waits for Ctrl+C and returns, allowing custom handling.
mod ctrlc;
pub use crate::ctrlc::{install_ctrlc_handler, wait_for_ctrlc};
