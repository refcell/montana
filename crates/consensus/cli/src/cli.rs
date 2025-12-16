//! CLI argument parsing and command execution.

use std::path::PathBuf;

use clap::Parser;

use crate::{CompressionAlgorithm, Mode};

/// Montana CLI.
///
/// Command-line interface for Montana, providing options for batch submission,
/// derivation, and roundtrip validation of L2 blocks with configurable compression.
///
/// # Examples
///
/// ```no_run
/// use montana_cli::Cli;
/// use clap::Parser;
///
/// let cli = Cli::parse();
/// println!("Mode: {:?}", cli.mode);
/// println!("Compression: {:?}", cli.compression);
/// ```
#[derive(Debug, Parser)]
#[command(name = "montana")]
#[command(
    about = "A modular and extensible duplex pipeline for L2 batch submission and derivation"
)]
pub struct Cli {
    /// Verbosity level (can be specified multiple times).
    ///
    /// Controls logging output: 0=WARN, 1=INFO, 2=DEBUG, 3+=TRACE
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Operation mode.
    ///
    /// Available modes: batch (default), derivation, roundtrip
    #[arg(short, long, default_value = "batch")]
    pub mode: Mode,

    /// Input JSON file containing transactions to batch submit.
    ///
    /// For batch mode: input blocks to compress and submit.
    /// For derivation mode: compressed batches to decompress.
    #[arg(short, long, default_value = "static/base_mainnet_blocks.json")]
    pub input: PathBuf,

    /// Output JSON file for the submitted batch.
    ///
    /// For batch mode: stores the compressed batch output.
    /// For derivation mode: stores the decompressed blocks.
    #[arg(short, long, default_value = "output.json")]
    pub output: PathBuf,

    /// Compression algorithm to use.
    ///
    /// Available algorithms: brotli (default), zlib, zstd, all
    #[arg(short, long, default_value = "brotli")]
    pub compression: CompressionAlgorithm,
}
