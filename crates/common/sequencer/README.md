# Sequencer

Sequencer buffer that bridges execution output to the batch submission pipeline.

## Overview

This crate provides [`ExecutedBlockBuffer`], a thread-safe buffer that:
- Receives executed L2 blocks from the execution layer
- Converts them to the `L2BlockData` format required by the batcher
- Implements the `BatchSource` trait from the channels crate

## Usage

```rust,ignore
use sequencer::ExecutedBlockBuffer;
use channels::BatchSource;

// Create a buffer with capacity for 256 blocks
let buffer = ExecutedBlockBuffer::new(256);

// Push executed blocks (from execution layer)
buffer.push(executed_block).await?;

// The buffer implements BatchSource for the batcher
let blocks = buffer.pending_blocks().await?;
```
