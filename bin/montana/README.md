# Montana Node

The Montana node binary for the Base stack.

## Architecture

The Montana node operates in a **sync + handoff** model:

1. **Sync Stage**: Catches up to the chain tip from a starting block
2. **Active Stage**: Sequencer/Validator roles process new blocks continuously

## Node Roles

- **Sequencer**: Executes blocks and submits batches to L1
- **Validator**: Derives blocks from L1 and validates execution
- **Dual** (default): Runs both sequencer and validator concurrently

## Usage

```bash
# Default: sync from checkpoint/genesis, run as dual (sequencer + validator)
montana --rpc-url <L2_RPC>

# Start from specific block
montana --rpc-url <L2_RPC> --start 1000000

# Run as sequencer only
montana --rpc-url <L2_RPC> --mode sequencer

# Run as validator only
montana --rpc-url <L2_RPC> --mode validator

# Skip sync (re-execution mode - for testing/debugging)
montana --rpc-url <L2_RPC> --skip-sync --start 1
```

## CLI Flags

| Flag | Description | Default |
|------|-------------|---------|
| `--rpc-url` / `-r` | L2 RPC URL (required) | - |
| `--mode` | Node role: sequencer, validator, dual | dual |
| `--start` | Starting block number | checkpoint or genesis |
| `--skip-sync` | Skip sync stage (for re-execution) | false |
| `--headless` | Run without TUI | false |
| `--batch-mode` | Batch submission: in-memory, anvil, remote | anvil |
| `--sync-threshold` | Blocks behind tip to consider synced | 10 |
| `--poll-interval-ms` | Chain tip poll interval | 2000 |
| `--checkpoint-path` | Checkpoint file location | ./data/checkpoint.json |
| `--checkpoint-interval` | Checkpoint save interval (seconds) | 10 |
| `--log-level` | Log level | info |
