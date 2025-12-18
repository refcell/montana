<h1 align="center">
<img src="./assets/header.png" alt="Montana" width="100%" align="center">
</h1>

<h4 align="center">
    An experimental, performant suite of Base stack components. Built in Rust.
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

Montana is an experimental, high-performance implementation of Base stack components. It provides both **sequencer** and **validator** node implementations, each with distinct execution and consensus layers.

### Architecture

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Montana                                         │
├─────────────────────────────────────┬───────────────────────────────────────┤
│            Sequencer                │            Validator                   │
├─────────────────────────────────────┼───────────────────────────────────────┤
│  ┌───────────────────────────────┐  │  ┌───────────────────────────────┐    │
│  │          Execution            │  │  │          Execution            │    │
│  │  • Block building             │  │  │  • Block execution            │    │
│  │  • Transaction ordering       │  │  │  • State verification         │    │
│  │  • State transitions          │  │  │  • State reconstruction       │    │
│  └───────────────────────────────┘  │  └───────────────────────────────┘    │
│  ┌───────────────────────────────┐  │  ┌───────────────────────────────┐    │
│  │          Consensus            │  │  │          Consensus            │    │
│  │  • Batch submission           │  │  │  • Derivation pipeline        │    │
│  │  • L1 blob/calldata posting   │  │  │  • L1 data retrieval          │    │
│  │  • Transaction management     │  │  │  • Batch decoding             │    │
│  └───────────────────────────────┘  │  └───────────────────────────────┘    │
└─────────────────────────────────────┴───────────────────────────────────────┘
```


**Execution** handles block processing using op-revm, providing state transitions for Base stack chains. The execution layer fetches blocks, executes transactions, and manages state via an in-memory database with RPC fallback.

**Consensus** manages the data availability layer through a trait-abstracted compression pipeline. For sequencers, this means batch submission to L1 via EIP-4844 blobs or calldata. For validators, this means derivation—fetching batches from L1, decompressing, and feeding blocks to execution.

For detailed documentation on the consensus pipeline architecture, see the [pipeline crate README](./crates/consensus/pipeline/README.md).

## Demo

<https://github.com/user-attachments/assets/ac59dcd6-a887-4e75-8e28-7317812c8b20>


> [!NOTE]
>
> The demo runs `just s` which is aliased to run the shadow binary.
> It shows pausing batch submission and resuming it by pressing "p".
> This demo uses a local anvil instance as the data availability provider.

## Overview

### Execution Layer

The execution layer processes blocks using op-revm:

```text
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│  BlockSource  │ ──▶  │ BlockExecutor │ ──▶  │   Database    │
│   (RPC/L1)    │      │  (op-revm)    │      │    (State)    │
└───────────────┘      └───────────────┘      └───────────────┘
```

### Consensus Layer

The consensus layer handles data availability via a duplex pipeline:

**Batch Submission (Sequencer)**
```text
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│  BatchSource  │ ──▶  │  Compressor   │ ──▶  │  BatchSink    │
│  (L2 Blocks)  │      │  (Brotli 11)  │      │  (L1 Blobs)   │
└───────────────┘      └───────────────┘      └───────────────┘
```

**Derivation (Validator)**
```text
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│ L1BatchSource │ ──▶  │ Decompressor  │ ──▶  │ L2BlockSink   │
│  (L1 Blobs)   │      │  (Brotli)     │      │  (L2 Blocks)  │
└───────────────┘      └───────────────┘      └───────────────┘
```

## Crates

<details>
<summary><strong>Binaries</strong></summary>

| Crate | Description |
|-------|-------------|
| [`montana`](./bin/montana) | Montana block executor node binary |

</details>

<details>
<summary><strong>Examples</strong></summary>

| Crate | Description |
|-------|-------------|
| [`shadow`](./examples/shadow) | Real-time batch submission and derivation monitoring TUI |
| [`analyze`](./examples/analyze) | Compression analyzer for benchmarking compression algorithms |
| [`fetcher`](./examples/fetcher) | Base L2 block fetcher utility |
| [`migrate`](./examples/migrate) | Reth MDBX to TrieDB database migration tool |

</details>

<details>
<summary><strong>Node</strong></summary>

| Crate | Description |
|-------|-------------|
| [`montana-node`](./crates/node/node) | Core node abstractions |
| [`montana-runtime`](./crates/node/runtime) | Runtime functions for node building and execution |
| [`montana-roles`](./crates/node/roles) | Role abstractions (sequencer, validator) |
| [`montana-checkpoint`](./crates/node/checkpoint) | Checkpoint persistence for node resumption |

</details>

<details>
<summary><strong>Consensus</strong></summary>

| Crate | Description |
|-------|-------------|
| [`montana-pipeline`](./crates/consensus/pipeline) | Core pipeline traits and types |
| [`montana-batcher`](./crates/consensus/batcher) | Batcher service for L2 batch submission orchestration |
| [`montana-batch-runner`](./crates/consensus/batch-runner) | Batch submission runner for L2 block streaming |
| [`montana-batch-context`](./crates/consensus/batch-context) | Sink and source abstractions for batch submission modes |
| [`montana-block-feeder`](./crates/consensus/block-feeder) | Block feeder for fetching and forwarding blocks |
| [`montana-derivation-runner`](./crates/consensus/derivation-runner) | Derivation pipeline runner for batch decompression |
| [`montana-txmgr`](./crates/consensus/txmgr) | Transaction manager for L1 batch submission (blob/calldata) |
| [`montana-local`](./crates/consensus/local) | Local file-based source and sink implementations |
| [`montana-anvil`](./crates/consensus/anvil) | Anvil integration for local testing and development |
| [`montana-brotli`](./crates/consensus/brotli) | Brotli compression implementation |
| [`montana-zlib`](./crates/consensus/zlib) | Zlib compression implementation |
| [`montana-zstd`](./crates/consensus/zstd) | Zstandard compression implementation |

</details>

<details>
<summary><strong>Execution</strong></summary>

| Crate | Description |
|-------|-------------|
| [`blocksource`](./crates/execution/blocksource) | Block source implementations for fetching Base stack blocks |
| [`database`](./crates/execution/database) | Database implementations for EVM state (TrieDB + RocksDB) |
| [`vm`](./crates/execution/vm) | Block executor using op-revm |
| [`runner`](./crates/execution/runner) | Block execution runner |
| [`migration`](./crates/execution/migration) | Reth MDBX database migration utilities |

</details>

<details>
<summary><strong>Common</strong></summary>

| Crate | Description |
|-------|-------------|
| [`chainspec`](./crates/common/chainspec) | Chain specification for Base stack chains |
| [`channels`](./crates/common/channel) | Channel utilities for async communication |
| [`primitives`](./crates/common/primitives) | Core primitive types |
| [`sequencer`](./crates/common/sequencer) | Sequencer buffer bridging execution to batch submission |

</details>

<details>
<summary><strong>Utilities</strong></summary>

| Crate | Description |
|-------|-------------|
| [`montana-cli`](./crates/utilities/cli) | CLI utilities and argument parsing |
| [`montana-tui`](./crates/utilities/tui) | TUI for the Montana node binary |
| [`montana-tui-common`](./crates/utilities/tui-common) | Common TUI utilities |
| [`montana-adapters`](./crates/utilities/adapters) | Adapter types for binary composition |
| [`montana-harness`](./crates/utilities/harness) | Test harness with synthetic transaction activity |

</details>

## Performance

Compression comparison using 31 Base mainnet blocks (5,766 transactions, 1,672,680 bytes raw):

| Algorithm | Compressed Size | Ratio |
|-----------|-----------------|-------|
| **Brotli** | 278,843 bytes | 16.7% |
| Zstd | 299,801 bytes | 17.9% |
| Zlib | 429,185 bytes | 25.7% |

Brotli provides the best compression ratio for L2 batch data, reducing the raw batch size by ~83.3%.

## Usage

### Quick Start

```sh
# Build the project
cargo build --release

# Run with just (recommended)
just --list              # Show all available commands
just build               # Build release binary
just test                # Run tests
just ci                  # Run full CI checks
```

### Running the Node

```sh
# Run Montana node (default: dual mode - sequencer + validator)
cargo run --release -p montana -- --rpc-url <L2_RPC>

# Run in sequencer mode
cargo run --release -p montana -- --rpc-url <L2_RPC> --mode sequencer

# Run in validator mode
cargo run --release -p montana -- --rpc-url <L2_RPC> --mode validator

# Run with local test harness (spawns anvil with synthetic activity)
just harness
```

### Examples

```sh
# Run the shadow TUI for monitoring
just shadow

# Run compression analyzer
cargo run -p analyze -- --help

# Run block fetcher
cargo run -p fetcher -- --help

# Migrate Reth MDBX database to TrieDB
just migrate <SOURCE_PATH> <DEST_PATH>
```

### Development

```sh
just fix                 # Auto-fix formatting and clippy issues
just check               # Run all checks (format, clippy, tests)
just bench               # Run compression benchmarks
just hack                # Check feature powerset
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
