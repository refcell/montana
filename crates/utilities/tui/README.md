# montana-tui

Terminal User Interface (TUI) for the Montana node binary.

## Overview

This crate provides a full-featured TUI for monitoring the Montana node in real-time. It displays a 4-pane layout showing:

- **Transaction Pool**: Real-time view of L2 transactions as they arrive
- **Block Builder**: L2 block building and batch submission activity
- **Batch Status**: Batch submission to L1 and compression metrics
- **Derivation**: Re-derivation of L2 blocks from L1 data with execution timing

## Features

- Real-time event streaming from the Montana binary
- Chain head tracking (unsafe, safe, finalized)
- Round-trip latency metrics and statistics
- Pause/resume functionality
- Log buffers for each component

## Usage

```rust,no_run
use montana_tui::{create_tui, TuiEvent};

// Create the TUI and a handle for sending events
let (tui, handle) = create_tui();

// Send events from the Montana binary
handle.send(TuiEvent::BlockBuilt {
    number: 100,
    tx_count: 5,
    size_bytes: 1024,
    gas_used: 21000,
});

// Run the TUI (blocks until user quits)
tui.run().expect("TUI failed");
```
