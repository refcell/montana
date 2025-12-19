# Benchmark Methodology

This document describes the methodology used for comparing Montana (Rust) against the OP Stack (Go).

## Principles

### 1. Apples-to-Apples Comparison

Both implementations must perform the **exact same logical operation**:
- Same input data (from shared fixtures)
- Same algorithm parameters (compression levels, buffer sizes)
- Same output validation

### 2. Optimized Builds

- **Rust**: `--release` profile with LTO enabled
- **Go**: Default build with `-ldflags="-s -w"` for stripped binaries

### 3. Statistical Rigor

- Minimum 5 iterations per benchmark
- Report mean, standard deviation, and percentiles (p50, p95, p99)
- Use statistical significance tests before claiming differences

### 4. Environment Control

- Document hardware specifications
- Pin CPU frequency when possible (disable turbo boost)
- Run benchmarks in isolation (no background processes)
- Warm up caches before measurement

## Benchmark Categories

### Compression Benchmarks

Compare compression algorithms with identical implementations:

| Algorithm | Montana Crate | OP Stack Implementation |
|-----------|---------------|------------------------|
| Brotli | `montana-brotli` | `github.com/andybalholm/brotli` |
| Zstd | `montana-zstd` | `github.com/klauspost/compress/zstd` |
| Zlib | `montana-zlib` | `compress/zlib` (stdlib) |

**Test Parameters**:
- Data sizes: 100 B, 1 KB, 10 KB, 100 KB
- Compression levels: Fast, Balanced

### Batch Building Benchmarks

Compare the process of building batches from L2 blocks:

| Stage | Montana | OP Stack |
|-------|---------|----------|
| Block encoding | `BatchCodec::encode()` | `batch.Derive()` |
| Compression | Configurable compressor | `ChannelOut.compress()` |
| Frame creation | N/A (Montana uses different framing) | `ChannelOut.OutputFrame()` |

**Test Parameters**:
- Block counts: 1, 10, 100
- Transactions per block: 5, 10

### Derivation Benchmarks

Compare the reverse process of deriving L2 data from L1:

| Stage | Montana | OP Stack |
|-------|---------|----------|
| Decompression | Compressor trait | `ChannelIn.decompress()` |
| Batch decoding | `BatchCodec::decode()` | `BatchReader.Read()` |
| Block validation | `DerivationRunner` | `derive.Pipeline` |

## Test Fixtures

### Generated Fixtures

`fixtures/blocks_*.json` - Synthetic block data:
```json
{
  "l1_origin": {
    "block_number": 12345,
    "hash_prefix": "0x..."
  },
  "parent_hash": "0x...",
  "blocks": [
    {
      "timestamp": 1000,
      "transactions": [
        {"data": "0x..."}
      ]
    }
  ]
}
```

### Realistic Fixtures

`fixtures/mainnet_blocks.json` - Real Base mainnet block data for production-like benchmarks.

## Measurement Tools

### Rust (Montana)

- **Framework**: Criterion.rs
- **Metrics**: Time, throughput, iterations
- **Output**: JSON + HTML reports in `target/criterion/`

### Go (OP Stack)

- **Framework**: `testing.B`
- **Metrics**: Time, memory, allocations
- **Analysis**: `benchstat` for statistical comparison

### Cross-Language

- **Command-line**: `hyperfine` for CLI tool comparison
- **Profiling**: `perf` on Linux, Instruments on macOS

## Avoiding Common Pitfalls

### Dead Code Elimination

Both languages optimize away unused computations:
- **Rust**: Use `black_box()` from Criterion
- **Go**: Use `testing.B.SetBytes()` or assign to package variable

### I/O Contamination

Never benchmark terminal output or file I/O unless specifically testing:
- Redirect output to `/dev/null`
- Use in-memory buffers

### Warm-up Effects

- Run warm-up iterations before measurement
- Criterion handles this automatically
- Go benchmarks should run `-benchtime=3s` minimum

### Memory Allocation

- Track allocations separately from timing
- Use `b.ReportAllocs()` in Go
- Consider using memory profilers for detailed analysis

## Reporting Results

### Required Information

1. **Hardware**: CPU model, cores, RAM, OS version
2. **Versions**: Rust version, Go version, dependency versions
3. **Commit**: Git commit hash for reproducibility
4. **Date**: When benchmarks were run

### Result Format

```
Benchmark                           Montana         OP Stack        Diff
────────────────────────────────────────────────────────────────────────
BrotliCompress/10KB                 45.2µs ± 2%     89.3µs ± 1%    -49%
ZstdCompress/10KB                   12.1µs ± 1%     15.8µs ± 2%    -23%
BatchBuild/100_blocks               234µs ± 3%      456µs ± 2%     -49%
```

### Statistical Significance

Only report differences as meaningful if:
- Confidence interval doesn't overlap
- p-value < 0.05 in t-test
- Difference exceeds noise threshold (typically 5%)

## Reproducing Results

```bash
# Clone the repository
git clone https://github.com/base/montana.git
cd montana

# Checkout specific commit
git checkout <commit-hash>

# Build Montana in release mode
cargo build --release

# Run Montana benchmarks
cargo bench --bench pipeline -- --save-baseline montana

# Run OP Stack benchmarks
cd benchmarks/op-stack
go test -bench=. -benchmem -count=5 | tee results.txt

# Compare with benchstat
benchstat results.txt
```
