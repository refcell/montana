# Fetcher

A simple binary that fetches Base mainnet L2 blocks and transactions from the public RPC endpoint and outputs them in the Montana JSON format.

## Usage

```bash
# Fetch 10 blocks starting from block 1000000
fetcher --start 1000000

# Fetch blocks 1000000 to 1000020
fetcher --start 1000000 --end 1000020

# Output to custom file
fetcher --start 1000000 --output blocks.json

# Use custom RPC URL
fetcher --start 1000000 --rpc https://your-rpc-endpoint.com
```

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
