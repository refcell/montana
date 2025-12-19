# Montana vs OP Stack Benchmarks

This directory contains benchmarks comparing Montana's performance against the OP Stack (op-batcher and op-node).

## Overview

The benchmarks focus on comparing equivalent operations between Montana (Rust) and the OP Stack (Go):

| Component | Montana | OP Stack | Operation |
|-----------|---------|----------|-----------|
| Compression | `montana-brotli` | `op-batcher/compressor` | Compress/decompress batch data |
| Batch Building | `montana-pipeline` | `op-node/rollup/derive` | Encode blocks into batches |
| Channel Assembly | `montana-pipeline` | `op-node/rollup/derive` | Assemble frames into channels |
| Derivation | `montana-derivation-runner` | `op-node/rollup/derive` | Derive L2 data from L1 |

## Directory Structure

```
benchmarks/
├── README.md           # This file
├── METHODOLOGY.md      # Detailed methodology documentation
├── fixtures/           # Shared test data for fair comparison
│   ├── blocks_1.json
│   ├── blocks_10.json
│   ├── blocks_100.json
│   └── batch_data.bin
├── op-stack/           # Go benchmarks for OP Stack components
│   ├── go.mod
│   ├── compression_test.go
│   ├── batch_building_test.go
│   └── derivation_test.go
├── compare.sh          # Run all benchmarks and compare
└── results/            # Benchmark results (gitignored)
```

## Quick Start

### Prerequisites

- Go 1.22+ (for OP Stack benchmarks)
- Rust 1.88+ (for Montana benchmarks)
- `just` command runner
- `hyperfine` (optional, for command-line benchmarking)

### Running Benchmarks

```bash
# Run all benchmarks and generate comparison
./benchmarks/compare.sh

# Run Montana benchmarks only
just bench

# Run OP Stack benchmarks only
cd benchmarks/op-stack && go test -bench=. -benchmem -count=5
```

## Methodology

### Fair Comparison Principles

1. **Identical Test Data**: Both implementations use the same input data from `fixtures/`
2. **Optimized Builds**: Rust uses `--release`, Go uses default optimizations
3. **Statistical Significance**: Multiple iterations with confidence intervals
4. **Component Isolation**: Benchmarks measure specific operations, not full system

### What We Measure

| Metric | Unit | Description |
|--------|------|-------------|
| Throughput | bytes/sec | Data processing rate |
| Latency | ns/op | Time per operation |
| Memory | B/op | Bytes allocated per operation |
| Allocations | allocs/op | Heap allocations per operation |

### Test Data Sizes

- **Small**: 1 block, 5 transactions (~500 bytes)
- **Medium**: 10 blocks, 10 transactions each (~10 KB)
- **Large**: 100 blocks, 10 transactions each (~100 KB)
- **Realistic**: 31 Base mainnet blocks (~1.67 MB)

## Results

See [RESULTS.md](./RESULTS.md) for the latest benchmark results.

### Summary (Latest Run)

```
                                Montana (Rust)    OP Stack (Go)    Speedup
────────────────────────────────────────────────────────────────────────────
Brotli Compression (10KB)       X.XX µs          X.XX µs          X.Xx
Zstd Compression (10KB)         X.XX µs          X.XX µs          X.Xx
Batch Building (100 blocks)     X.XX µs          X.XX µs          X.Xx
Channel Assembly                X.XX µs          X.XX µs          X.Xx
────────────────────────────────────────────────────────────────────────────
```

## Contributing

When adding new benchmarks:

1. Add test fixtures to `fixtures/` if needed
2. Implement both Rust and Go versions
3. Document methodology in `METHODOLOGY.md`
4. Update results in `RESULTS.md`

## License

MIT License - see the [LICENSE](../LICENSE) file.
