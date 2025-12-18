# montana-runtime

Runtime functions for building and running the Montana node.

## Overview

This crate extracts the node building logic, runtime management, and TUI integration from the binary crate to keep the binary minimal. It provides the core runtime functionality for:

- Building nodes with proper configuration
- Running nodes in headless or TUI mode
- Integrating block producers, sequencers, and validators
- Managing the node lifecycle

## Core Functions

| Function | Description |
|----------|-------------|
| `run_headless` | Run the node in headless mode (without TUI) |
| `run_with_tui` | Run the node with the terminal user interface |
| `build_node` | Build a node with the appropriate block producer |
| `mode_to_node_role` | Convert CLI mode to node role |

## Usage

```rust,ignore
use montana_runtime::{run_headless, run_with_tui};
use montana_cli::MontanaCli;

// Parse CLI arguments
let cli = MontanaCli::parse();

// Run in appropriate mode
if cli.headless {
    run_headless(cli).await?;
} else {
    run_with_tui(cli)?;
}
```

## Architecture

The runtime handles:

1. **Provider Setup**: Creates Alloy providers for RPC communication
2. **Block Producer**: Sets up RPC or channel-based block producers
3. **Batch Context**: Configures batch submission (in-memory, Anvil, remote)
4. **Role Configuration**: Builds sequencer/validator based on CLI flags
5. **TUI Integration**: Wires up callbacks for TUI visibility

## License

Licensed under the MIT license.
