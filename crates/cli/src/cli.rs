//! CLI argument parsing and command execution.

use std::path::PathBuf;

use clap::Parser;

use crate::CompressionAlgorithm;

/// Montana CLI.
#[derive(Debug, Parser)]
#[command(name = "montana")]
#[command(
    about = "A modular and extensible duplex pipeline for L2 batch submission and derivation"
)]
pub struct Cli {
    /// Verbosity level (can be specified multiple times)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Input JSON file containing transactions to batch submit.
    #[arg(short, long, default_value = "static/base_mainnet_blocks.json")]
    pub input: PathBuf,

    /// Output JSON file for the submitted batch.
    #[arg(short, long, default_value = "output.json")]
    pub output: PathBuf,

    /// Compression algorithm to use.
    #[arg(short, long, default_value = "brotli")]
    pub compression: CompressionAlgorithm,
}
