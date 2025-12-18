# migrate

Migrate Reth MDBX database to TrieDB.

## Overview

This tool reads account and storage state from a Reth MDBX database and writes it to a new TrieDB database. It's useful for migrating existing chain state into Montana's storage format.

## Usage

```bash
cargo run -p migrate -- --source /path/to/reth/db --dest /path/to/triedb
```

## CLI Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--source` | `-s` | Path to source Reth MDBX database | (required) |
| `--dest` | `-d` | Path to destination TrieDB database | (required) |
| `--batch-size` | - | Entries per batch commit | 10,000 |
| `--log-interval` | - | Entries between progress logs | 100,000 |

## Example

```bash
# Basic migration
migrate --source ./reth/db --dest ./montana/triedb

# With custom batch size for large databases
migrate --source ./reth/db --dest ./montana/triedb --batch-size 50000 --log-interval 500000
```

## Output

The tool logs:
- Total accounts migrated
- Total storage slots migrated
- Any errors encountered
- Final state root

## License

Licensed under the MIT license.
