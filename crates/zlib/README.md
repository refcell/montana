# montana-zlib

Zlib compression implementation for the Montana batch submission pipeline.

## Overview

This crate provides a `ZlibCompressor` that implements the `Compressor` trait from `montana-pipeline`. It uses the DEFLATE compression algorithm via the `flate2` crate.

## Usage

```rust
use montana_zlib::ZlibCompressor;
use montana_pipeline::Compressor;

// Create a compressor with default settings
let compressor = ZlibCompressor::default();

// Or use presets
let fast = ZlibCompressor::fast();           // Level 1 - fastest
let balanced = ZlibCompressor::balanced();   // Level 6 - good balance
let best = ZlibCompressor::best();           // Level 9 - best compression

// Compress data
let data = b"Hello, World!";
let compressed = compressor.compress(data).unwrap();

// Decompress data
let decompressed = compressor.decompress(&compressed).unwrap();
assert_eq!(decompressed, data);
```

## Compression Levels

Zlib supports compression levels 0-9:
- **0**: No compression (store only)
- **1**: Fastest compression
- **6**: Default/balanced compression
- **9**: Best compression (slowest)
