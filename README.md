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
  <a href="#performance">Performance</a> •
  <a href="#usage">Usage</a> •
  <a href="#contributing">Contributing</a> •
  <a href="#provenance">Provenance</a> •
  <a href="#license">License</a>
</p>

> [!CAUTION]
> Montana is under active development and is not ready for production use.

## What's Montana?

Montana is an experimental, high-performance implementation of a minimal L2 stack written entirely in Rust. It provides a complete L2 stack comprising both sequencer and validator node implementations, each with distinct execution and consensus layers. The execution layer processes blocks using op-revm, providing state transitions for Base stack chains through block fetching, transaction execution, and state management via an in-memory database with RPC fallback. The consensus layer manages the data availability layer through a trait-abstracted compression pipeline supporting Brotli, Zstd, and Zlib compression algorithms.

Montana includes a local simulation harness that orchestrates anvil instances for both L1 and L2 chains, enabling full end-to-end testing of the batch submission and derivation pipeline without external infrastructure. The sequencer batches L2 blocks and submits them to L1 via EIP-4844 blobs or legacy calldata, while the validator derives batches from L1, decompresses them, and re-executes blocks to reconstruct the canonical chain state. The architecture is fully modular with trait-abstracted sources and sinks, allowing batch data to flow through local files, anvil chains, or production L1 endpoints.

For detailed documentation on the consensus pipeline architecture, see the [pipeline crate README](./crates/consensus/pipeline/README.md).

## Demo

<https://github.com/user-attachments/assets/23c68bad-61ea-49b9-9c22-b3f574acda7c>


> [!NOTE]
>
> The demo runs `just harness-fast` which is aliased to run the montana binary.
>
> This demo shows an L2 anvil chain that a sequencer takes transactions from
> and batch submits them to an anvil L1 chain. A validator then derives these
> batches and then re-executes them, resulting in the canonical finalized L2 chain.


## Performance

### Batch Submission → Derivation Round Trip

Full pipeline benchmarks measured across 1,000 batches on local anvil infrastructure:

```
                                    p50         p95         p99
  ────────────────────────────────────────────────────────────────
  Batch Submission (L2 → L1)       12ms        18ms        24ms
  Derivation (L1 → L2)              8ms        14ms        19ms
  Full Round Trip                  21ms        34ms        45ms
  ────────────────────────────────────────────────────────────────
```

### Block Execution

Measured using op-revm across 10,000 Base mainnet blocks:

```
                                    p50         p95         p99
  ────────────────────────────────────────────────────────────────
  Block Execution                  1.2ms       3.8ms       7.1ms
  State Commitment                 0.4ms       0.9ms       1.6ms
  ────────────────────────────────────────────────────────────────
```

### Compression

Benchmarked with 31 Base mainnet blocks (5,766 transactions, 1.67 MB raw):

```
  ┌─────────────────────────────────────────────────────────────┐
  │                                                             │
  │  Brotli   ████████████████░░░░░░░░░░░░░░░░░░░░░░  16.7%    │
  │  Zstd     ██████████████████░░░░░░░░░░░░░░░░░░░░  17.9%    │
  │  Zlib     ██████████████████████████░░░░░░░░░░░░  25.7%    │
  │                                                             │
  └─────────────────────────────────────────────────────────────┘
```

**Brotli** achieves the best compression at **83.3% reduction**, making it the default for batch submission.

## Usage

Simulate the entire L2 stack locally using anvil instances for both L1 and L2 chains.

```sh
just harness-fast
```

> [!TIP]
> See the [Justfile](./Justfile) for other useful commands, including `just ci` to run all CI checks locally.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Provenance

[@danyalprout](https://github.com/danyalprout) and [@refcell](https://github.com/refcell) built this in a week during a [Base](https://github.com/base) Hackathon.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
