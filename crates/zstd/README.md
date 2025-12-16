# montana-zstd

Zstandard (zstd) compression implementation for the Montana batch submission pipeline.

## Overview

This crate provides a `ZstdCompressor` that implements the `Compressor` trait from `montana-pipeline`. Zstandard offers an excellent balance of compression ratio and speed, making it ideal for L2 batch compression.

## Usage

```rust
use montana_zstd::ZstdCompressor;
use montana_pipeline::Compressor;

// Create a compressor with default settings
let compressor = ZstdCompressor::default();

// Or use presets
let fast = ZstdCompressor::fast();           // Level 1 - fastest
let balanced = ZstdCompressor::balanced();   // Level 3 - good balance
let best = ZstdCompressor::best();           // Level 19 - best compression

// Compress data
let data = b"Hello, World!";
let compressed = compressor.compress(data).unwrap();

// Decompress data
let decompressed = compressor.decompress(&compressed).unwrap();
assert_eq!(decompressed, data);
```

## Compression Levels

Zstandard supports compression levels 1-22:
- **1**: Fastest compression
- **3**: Default/balanced compression
- **19**: Best compression (zstd --ultra not enabled)
- **22**: Maximum compression (very slow)

## Why Zstandard?

Zstandard was developed by Facebook and offers:
- **Fast decompression**: Critical for L2 derivation
- **Good compression ratios**: Competitive with Brotli
- **Dictionary support**: Can achieve even better ratios with pre-trained dictionaries
- **Widely adopted**: Used by many blockchain projects
