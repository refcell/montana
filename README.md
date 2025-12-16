<h1 align="center">
<img src="./assets/header.png" alt="Montana" width="100%" align="center">
</h1>

<h4 align="center">
    A modular and extensible duplex pipeline for L2 batch submission and derivation. Built in Rust.
</h4>

<p align="center">
  <a href="https://github.com/base/montana/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/base/montana/ci.yml?style=flat&labelColor=1C2C2E&label=ci&color=BEC5C9&logo=GitHub%20Actions&logoColor=BEC5C9" alt="CI"></a>
  <a href="https://github.com/base/montana/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?style=flat&labelColor=1C2C2E&color=BEC5C9&logo=googledocs&label=license&logoColor=BEC5C9" alt="License"></a>
</p>

<p align="center">
  <a href="#whats-montana">What's Montana?</a> •
  <a href="#overview">Overview</a> •
  <a href="#crates">Crates</a> •
  <a href="#performance">Performance</a> •
  <a href="#usage">Usage</a> •
  <a href="#contributing">Contributing</a> •
  <a href="#license">License</a>
</p>

## What's Montana?

Montana is a minimal, trait-abstracted compression pipeline for L2 batch submission and derivation.
It provides an ecosystem of extensible, low-level crates that compose into components for the OP Stack
batcher and derivation pipelines.

## Overview

Montana is a unidirectional data pipeline:

```
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│  DataSource   │ ──▶  │  Compressor   │ ──▶  │  DataSink     │
│  (L2 Blocks)  │      │  (Brotli 11)  │      │  (L1 Blobs)   │
└───────────────┘      └───────────────┘      └───────────────┘
```

The inverse pipeline for derivation:

```
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│  DataSource   │ ──▶  │ Decompressor  │ ──▶  │  DataSink     │
│  (L1 Blobs)   │      │  (Brotli)     │      │  (L2 Blocks)  │
└───────────────┘      └───────────────┘      └───────────────┘
```

## Crates

**Binaries**

- [`montana`](./bin/montana): The batch submitter execution extension.
- [`analyze`](./bin/analyze): Compression analyzer for comparing algorithm performance.

**Pipeline**

- [`montana-pipeline`](./crates/pipeline): Core pipeline types and traits.
- [`montana-local`](./crates/local): Local file-based source and sink implementations.
- [`montana-cli`](./crates/cli): CLI utilities and argument parsing.
- [`montana-brotli`](./crates/brotli): Brotli compression implementation.

## Performance

Compression comparison using 10 Base mainnet blocks (1,233 transactions, 616,189 bytes raw):

| Algorithm | Compressed Size | Ratio |
|-----------|-----------------|-------|
| **Brotli** | 69,765 bytes | 11.3% |
| Zstd | 72,940 bytes | 11.8% |
| Zlib | 130,116 bytes | 21.1% |

Brotli provides the best compression ratio for L2 batch data, reducing the raw batch size by ~88.7%.

## Usage

```sh
# Build the project
cargo build --release

# Run the compression analyzer
cargo run -p analyze -- --help

# Run the batch submitter
cargo run -p montana -- --help
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
