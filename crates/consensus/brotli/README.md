# montana-brotli

Brotli compression implementation for the Montana batch submission pipeline.

## Overview

This crate provides a `BrotliCompressor` that implements the `Compressor` trait from `montana-pipeline`. Brotli provides excellent compression ratios, especially for text-like data such as RLP-encoded transactions.

## Usage

```rust
use montana_brotli::BrotliCompressor;
use montana_pipeline::Compressor;

// Create a compressor with default settings
let compressor = BrotliCompressor::default();

// Or use presets
let fast = BrotliCompressor::fast();                    // Level 1 - fastest
let balanced = BrotliCompressor::balanced();            // Level 6 - good balance
let max = BrotliCompressor::max_compression();          // Level 11 - maximum compression

// Compress data
let data = b"Hello, World!";
let compressed = compressor.compress(data).unwrap();

// Decompress data
let decompressed = compressor.decompress(&compressed).unwrap();
assert_eq!(decompressed, data);
```

## Compression Levels

Brotli supports compression levels 0-11:
- **0**: No compression (store only)
- **1**: Fastest compression
- **6**: Balanced compression
- **11**: Maximum compression (default, optimized for L2 batch submission)

## Features

- High compression ratios with configurable levels (0-11)
- Configurable window size for memory/compression tradeoff (default: 4MB, log2 = 22)
- Deterministic compression for reproducible batches
- Default configuration optimized for L2 batch submission (level 11, 4MB window)
