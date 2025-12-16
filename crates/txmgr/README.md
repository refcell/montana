# montana-txmgr

Transaction manager for L1 batch submission with EIP-4844 blob and calldata support.

## Overview

`montana-txmgr` provides a robust, production-ready transaction management system for submitting batch data to L1. It handles:

- **Nonce management** - Thread-safe nonce tracking with automatic sync
- **Gas estimation** - Dynamic fee calculation with configurable bumping
- **Transaction building** - Support for both calldata and blob transactions
- **Submission retry logic** - Automatic fee bumping and error handling
- **Receipt monitoring** - Polling for confirmation with configurable depth

## Core Components

| Component | Description |
|-----------|-------------|
| `TxManagerConfig` | Configuration with sane defaults |
| `TxCandidate` | Transaction candidate (calldata or blob) |
| `TxReceipt` | Confirmed transaction receipt |
| `NonceTracker` | Thread-safe nonce management |
| `GasCaps` | Gas fee caps for EIP-1559 transactions |
| `TxBuilder` | Transaction construction |
| `TxSubmitter` | Mempool submission with retry |
| `TxMonitor` | Receipt polling and confirmation |
| `SendState` | Per-transaction state machine |

## Usage

```rust
use montana_txmgr::{TxManagerConfig, TxCandidate, TxBuilder};
use alloy::primitives::{Address, Bytes};

// Create a calldata transaction candidate
let candidate = TxCandidate::calldata(
    Address::ZERO, // batch inbox address
    Bytes::from_static(&[0x01, 0x02, 0x03]),
);

// Or create a blob transaction
// let sidecar = BlobTransactionSidecar::new(...);
// let candidate = TxCandidate::blob(inbox_address, sidecar);

// Configure the transaction manager
let config = TxManagerConfig::builder()
    .num_confirmations(10)
    .resubmission_timeout(std::time::Duration::from_secs(48))
    .build();
```

## Configuration

Default configuration values:

| Setting | Default | Description |
|---------|---------|-------------|
| `num_confirmations` | 10 | Required block confirmations |
| `receipt_query_interval` | 12s | Receipt polling interval |
| `network_timeout` | 10s | RPC call timeout |
| `resubmission_timeout` | 48s | Fee bump interval |
| `not_in_mempool_timeout` | 2m | Max mempool wait time |
| `safe_abort_nonce_too_low_count` | 3 | Nonce errors before abort |
| `fee_limit_multiplier` | 5x | Fee cap multiplier |
| `price_bump_percent` | 10% | Regular tx fee bump |
| `blob_price_bump_percent` | 100% | Blob tx fee bump |

## Error Handling

Errors are classified for retry logic:

- **Retryable**: `Rpc`, `Underpriced`, `NonceTooLow`
- **Non-retryable**: `InsufficientFunds`, `AlreadyReserved`, `NonceTooLowAbort`

## License

Licensed under the MIT license.
