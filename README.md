<h1 align="center">
<img src="./assets/header.png" alt="Montana" width="100%" align="center">
</h1>

<h4 align="center">
    A modular and extensible duplex pipeline for L2 batch submission and derivation. Built in Rust.
</h4>

<p align="center">
  <a href="https://crates.io/crates/montana"><img src="https://img.shields.io/crates/v/montana.svg?style=flat&labelColor=1C2C2E&color=BEC5C9" alt="Crates.io"></a>
  <a href="https://github.com/base/montana/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/base/montana/ci.yml?style=flat&labelColor=1C2C2E&label=ci&color=BEC5C9&logo=GitHub%20Actions&logoColor=BEC5C9" alt="CI"></a>
  <a href="https://github.com/base/montana/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-d1d1f6.svg?style=flat&labelColor=1C2C2E&color=BEC5C9&logo=googledocs&label=license&logoColor=BEC5C9" alt="License"></a>
  <img src="https://img.shields.io/badge/chain-base-blue?style=flat&labelColor=1C2C2E" alt="Chain: Base">
</p>

<p align="center">
  <a href="#whats-montana">What's Montana?</a> •
  <a href="#demo">Demo</a> •
  <a href="#overview">Overview</a> •
  <a href="#crates">Crates</a> •
  <a href="#performance">Performance</a> •
  <a href="#usage">Usage</a> •
  <a href="#contributing">Contributing</a> •
  <a href="#license">License</a>
</p>

> [!CAUTION]
> Montana is under active development and is not ready for production use.

## What's Montana?

Montana is a minimal, trait-abstracted compression pipeline for L2 batch submission and derivation. It implements a three-stage data flow architecture where each stage is defined by a Rust trait, enabling independent implementation swapping without modifying the pipeline core.

The batch submission direction collects L2 blocks from a `BatchSource`, which polls an execution client for pending transactions and block metadata. Each block carries a timestamp and a vector of `RawTransaction` values containing opaque RLP-encoded bytes. The source also provides the current L1 origin block number and truncated hashes for chain validation. These blocks accumulate until the pipeline determines a batch should be formed, at which point it constructs a 67-byte `BatchHeader` containing the wire format version, monotonically increasing batch sequence number, L1 epoch reference, 20-byte hash prefixes for both L1 origin and parent L2 block, the first block timestamp, and a block count. The `BatchCodec` trait serializes the header followed by block data using little-endian encoding, with each block containing a 2-byte timestamp delta, 2-byte transaction count, and transactions prefixed by 3-byte length fields.

The serialized batch passes through a `Compressor` trait implementation. Montana defaults to Brotli at level 11 with a 4MB sliding window, which achieves roughly 88% size reduction on typical L2 transaction data. The compressor must be deterministic since the same input must always produce identical output for consensus verification during derivation. The compressed payload is wrapped with a version byte and handed to a `BatchSink`, which submits the data to L1 via EIP-4844 blob transactions. The sink abstracts submission details including gas price limits, retry logic with exponential backoff, and confirmation waiting. A `FallbackSink` combinator enables automatic degradation to calldata submission when blob gas becomes prohibitively expensive.

The derivation direction inverts this flow. An `L1BatchSource` fetches compressed batches from L1 blob or calldata, the same compressor decompresses the payload, and the codec decodes blocks which are fed to an `L2BlockSink` for execution. The pipeline validates batch sequence numbers to detect gaps and verifies header version compatibility.

Configuration constants define operational boundaries. Maximum compressed batch size is 128KB to fit within a single blob. Minimum batch size is 1KB to avoid dust submissions. The default submission interval aligns with L1 block time at 12 seconds. The sequencing window spans 3600 L1 blocks, and safe head confirmation requires 12 blocks.

For detailed documentation on the pipeline architecture, configuration, batching model, and streaming considerations, see the [pipeline crate README](./crates/pipeline/README.md).

## Demo

<https://github.com/user-attachments/assets/ac59dcd6-a887-4e75-8e28-7317812c8b20>


> [!NOTE]
>
> The demo runs `just s` which is aliased to run the shadow binary.
> It shows pausing batch submission and resuming it by pressing "p".
> This demo uses a local anvil instance as the data availability provider.

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
- [`shadow`](./bin/shadow): Real-time batch submission and derivation monitoring TUI.
- [`fetcher`](./bin/fetcher): Base L2 block fetcher.

**Pipeline**

- [`montana-pipeline`](./crates/pipeline): Core pipeline types and traits.
- [`montana-batcher`](./crates/batcher): Batcher service for L2 batch submission orchestration.
- [`montana-txmgr`](./crates/txmgr): Transaction manager for L1 batch submission with blob and calldata support.
- [`montana-local`](./crates/local): Local file-based source and sink implementations.
- [`montana-cli`](./crates/cli): CLI utilities and argument parsing.
- [`montana-brotli`](./crates/brotli): Brotli compression implementation.
- [`montana-zlib`](./crates/zlib): Zlib compression implementation.
- [`montana-zstd`](./crates/zstd): Zstandard compression implementation.

## Performance

Compression comparison using 31 Base mainnet blocks (5,766 transactions, 1,672,680 bytes raw):

| Algorithm | Compressed Size | Ratio |
|-----------|-----------------|-------|
| **Brotli** | 278,843 bytes | 16.7% |
| Zstd | 299,801 bytes | 17.9% |
| Zlib | 429,185 bytes | 25.7% |

Brotli provides the best compression ratio for L2 batch data, reducing the raw batch size by ~83.3%.

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
