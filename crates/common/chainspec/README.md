# chainspec

Chain specification for Base stack chains.

## Overview

This crate provides chain specifications including hardfork activation timestamps and helpers for deriving the correct `OpSpecId` from a block timestamp.

## Core Types

| Type | Description |
|------|-------------|
| `Chain` | Chain specification containing chain ID, name, and hardfork timestamps |
| `Hardforks` | Hardfork activation timestamps for each Base stack fork |
| `Hardfork` | Enum of Base stack hardforks (Bedrock, Canyon, Delta, etc.) |

## Supported Hardforks

- **Bedrock** - Initial Base stack launch
- **Canyon** - First post-Bedrock hardfork
- **Delta** - Span batch support
- **Ecotone** - 4844 blob support
- **Fjord** - RIP-7212 secp256r1 precompile
- **Granite** - Various bug fixes
- **Holocene** - EIP-1559 configurability
- **Isthmus** - Future hardfork
- **Jovian** - Future hardfork

## Usage

```rust,ignore
use chainspec::{Chain, Hardfork, BASE_MAINNET};

// Get the spec ID for a given timestamp
let spec_id = BASE_MAINNET.spec_id_at_timestamp(1710374401);

// Check if a hardfork is active
if BASE_MAINNET.is_active(Hardfork::Ecotone, timestamp) {
    // Ecotone-specific logic
}

// Get the currently active hardfork
let active = BASE_MAINNET.active_hardfork(timestamp);
```

## Pre-defined Chains

- `BASE_MAINNET` - Base Mainnet (chain ID 8453)

## License

Licensed under the MIT license.
