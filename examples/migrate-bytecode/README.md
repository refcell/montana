# migrate-bytecode

Migrate Reth's `Bytecodes` table to a RocksDB key-value store.

## Usage

```bash
cargo run -p migrate-bytecode -- --source /path/to/reth/db --dest /path/to/rocksdb
```

## Options

- `--source`, `-s`: Path to the source Reth MDBX database
- `--dest`, `-d`: Path to the destination RocksDB database
- `--batch-size`: Number of entries to process before flushing (default: 10000)
- `--log-interval`: Number of entries between progress logs (default: 100000)

## Output Format

The RocksDB database stores bytecode entries as:
- **Key**: Code hash (32 bytes, B256)
- **Value**: Raw bytecode bytes
