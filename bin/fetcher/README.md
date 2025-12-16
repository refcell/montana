# Fetcher

A binary that fetches Base mainnet L2 blocks and transactions from the public RPC endpoint and outputs them in the Montana JSON format.

## Usage

```bash
# Fetch 10 blocks starting from block 1000000 (defaults to fetched_blocks.json)
fetcher --start 1000000

# Fetch blocks 1000000 to 1000020
fetcher --start 1000000 --end 1000020

# Output to custom file
fetcher --start 1000000 --output blocks.json

# Use custom RPC URL (default: https://mainnet.base.org)
fetcher --start 1000000 --rpc https://your-rpc-endpoint.com

# Enable verbose logging (-v for INFO, -vv for DEBUG, -vvv for TRACE)
fetcher --start 1000000 -v

# Combine options with verbose logging
fetcher --start 1000000 --end 1000020 --output my-blocks.json -vv
```

## CLI Options

| Flag | Short | Description | Default |
|------|-------|-------------|---------|
| `--start` | `-s` | Starting block number to fetch (required) | - |
| `--end` | `-e` | Ending block number (inclusive) | `start + 9` |
| `--output` | `-o` | Output JSON file path | `fetched_blocks.json` |
| `--rpc` | `-r` | RPC URL for Base mainnet | `https://mainnet.base.org` |
| `--verbose` | `-v` | Verbosity level (repeat for more: -v, -vv, -vvv) | WARN |

## Output Format

The output JSON matches the Montana input format:

```json
{
    "l1_origin": {
        "block_number": 12345678,
        "hash_prefix": "0x..."
    },
    "parent_hash": "0x...",
    "blocks": [
        {
            "timestamp": 1700000000,
            "transactions": [
                {"data": "0x..."}
            ]
        }
    ]
}
```

### Implementation Notes

- **L1 Origin**: Currently stubbed with placeholder values (`block_number: 0`, zero hash). A complete implementation would require additional lookups to determine the L1 origin for each L2 block range.
- **Parent Hash**: Correctly extracted from the first block's parent and truncated to 20 bytes (40 hex characters) as required by Montana's format.
- **Transactions**: Transaction data is extracted from the `input` field of each transaction in the block.
