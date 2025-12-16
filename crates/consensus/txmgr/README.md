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

### Creating Transaction Candidates

```rust
use montana_txmgr::{TxCandidate, TxManagerConfig};
use alloy::primitives::{Address, Bytes, U256};
use alloy::consensus::BlobTransactionSidecar;

// Create a calldata transaction candidate
let candidate = TxCandidate::calldata(
    Address::ZERO, // batch inbox address
    Bytes::from_static(&[0x01, 0x02, 0x03]),
)
.with_gas_limit(100_000)  // Optional gas limit override
.with_value(U256::ZERO);   // Optional value (default is 0)

// Or create a blob transaction
let sidecar = BlobTransactionSidecar::default(); // Create with your blob data
let candidate = TxCandidate::blob(
    Address::ZERO, // batch inbox address
    sidecar,
);

// Check transaction type
assert!(candidate.is_blob());
```

### Configuring the Transaction Manager

```rust
use std::time::Duration;

let config = TxManagerConfig::builder()
    .num_confirmations(10)
    .resubmission_timeout(Duration::from_secs(48))
    .network_timeout(Duration::from_secs(10))
    .price_bump_percent(10)
    .blob_price_bump_percent(100)
    .build();
```

### Transaction Receipt

```rust
use montana_txmgr::TxReceipt;

// After successful submission, you receive a receipt
// let receipt: TxReceipt = ...

// Calculate total cost (base gas + blob gas if applicable)
let total_wei = receipt.total_cost();
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
