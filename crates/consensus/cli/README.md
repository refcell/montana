# montana-cli

CLI utilities and argument parsing for Montana, a modular and extensible duplex pipeline for L2 batch submission and derivation.

## Overview

This crate provides the command-line interface components for Montana, including:

- **CLI Argument Parsing**: Main CLI struct with argument definitions
- **Operation Modes**: Batch submission, derivation, and roundtrip validation
- **Compression Algorithms**: Support for Brotli, Zlib, and Zstandard
- **Tracing Utilities**: Logging initialization with configurable verbosity levels

## Provided Types

### `Cli`

The main CLI argument parser built with `clap`. Provides the following options:

- `-v, --verbose`: Verbosity level (repeatable flag)
- `-m, --mode`: Operation mode (batch, derivation, or roundtrip)
- `-i, --input`: Input JSON file path
- `-o, --output`: Output JSON file path
- `-c, --compression`: Compression algorithm to use

### `Mode`

Operation mode enum with three variants:

- `Batch` (default): Compress and submit L2 blocks
- `Derivation`: Decompress and derive L2 blocks from compressed batches
- `Roundtrip`: Batch submission followed by derivation with validation

### `CompressionAlgorithm`

Compression algorithm selection enum:

- `Brotli` (default): Brotli compression
- `Zlib`: Zlib (DEFLATE) compression
- `Zstd`: Zstandard compression
- `All`: Run all compression algorithms and compare results

Provides the `all_algorithms()` method to iterate over single compression algorithms (excludes `All`).

### `init_tracing`

Function to initialize the tracing subscriber with configurable verbosity:

- `0`: WARN level
- `1`: INFO level
- `2`: DEBUG level
- `3+`: TRACE level

Respects the `RUST_LOG` environment variable for custom filter configuration.

## Usage

```rust
use montana_cli::{Cli, Mode, CompressionAlgorithm, init_tracing};
use clap::Parser;

// Parse CLI arguments
let cli = Cli::parse();

// Initialize tracing based on verbosity
init_tracing(cli.verbose);

// Access CLI options
match cli.mode {
    Mode::Batch => {
        // Batch submission logic
    }
    Mode::Derivation => {
        // Derivation logic
    }
    Mode::Roundtrip => {
        // Roundtrip validation logic
    }
}
```
