# montana-brotli

Brotli compression implementation for Montana.

This crate provides a `BrotliCompressor` that implements the `Compressor` trait
from `montana-pipeline` using the Brotli compression algorithm.

## Features

- High compression ratios (level 1-11)
- Configurable window size for memory/compression tradeoff
- Deterministic compression for reproducible batches
- Default configuration optimized for L2 batch submission (level 11, 4MB window)
