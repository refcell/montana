# shadow

A TUI (Terminal User Interface) for shadowing a live chain and simulating batch submission to L1.

## Overview

The `shadow` binary provides real-time visualization of the Montana batch submission and derivation pipeline by streaming blocks directly from an L2 RPC endpoint. It displays a split-pane interface showing:

- **Header**: Stats and metrics including chain head, current block, batch counts, compression ratios, and health status
- **Left Pane**: Batch submission logs showing block fetching and compression progress
- **Right Pane**: Derivation logs showing decompression and validation

Since this is a shadow/simulation mode, batches are not actually posted to L1. Instead, the compressed batch data is passed directly from the batch submission side to the derivation side for validation.

## Usage

```bash
# Shadow Base mainnet (default RPC)
shadow

# Shadow with a custom RPC endpoint
shadow --rpc https://mainnet.base.org

# Start from a specific block number
shadow --start 12345678

# Use zstd compression
shadow --compression zstd

# Adjust polling and batching parameters
shadow --poll-interval 1000 --max-blocks-per-batch 20 --target-batch-size 65536

# Enable verbose logging
shadow -v
```

## Keyboard Controls

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit the application |
| `p` | Pause/Resume simulation |
| `r` | Reset and restart |

## Command Line Options

| Option | Default | Description |
|--------|---------|-------------|
| `-r, --rpc` | `https://mainnet.base.org` | RPC URL for the L2 chain |
| `-s, --start` | Latest block | Starting block number |
| `-c, --compression` | `brotli` | Compression algorithm (`brotli`, `zlib`, `zstd`) |
| `--poll-interval` | `2000` | Block polling interval in milliseconds |
| `--max-blocks-per-batch` | `10` | Maximum blocks per batch |
| `--target-batch-size` | `131072` | Target batch size in bytes before submitting |
| `-v, --verbose` | - | Increase verbosity level |

## Architecture

```
+-------------------------------------------+
|            Stats & Metrics                |
|  Chain Head | Current Block | Lag         |
|  Batches | Blocks | Bytes | Ratio | Health|
+-------------------+-----------------------+
|                   |                       |
|  Batch Submission |     Derivation        |
|  (RPC Streaming)  |   (Decompression)     |
|                   |                       |
|  [INFO] Block #N  |  [INFO] Derived #0    |
|  [INFO] Batch #0  |  [INFO] Derived #1    |
|        ...        |         ...           |
|                   |                       |
+-------------------+-----------------------+
```

The TUI uses a vertical split where the header occupies the top 1/4 of the screen, and the bottom 3/4 is split horizontally between the two log panes (each taking 50% of the width).

## How It Works

1. **Block Streaming**: Connects to the specified RPC endpoint and fetches blocks starting from the latest (or specified) block number
2. **Batch Accumulation**: Accumulates blocks until either the block count or byte size threshold is reached
3. **Compression**: Compresses the batch using the selected algorithm (brotli, zlib, or zstd)
4. **Derivation**: Passes the compressed batch to the derivation side for decompression and validation
5. **Metrics**: Tracks compression ratios, block counts, and overall pipeline health
