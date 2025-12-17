//! Executed block buffer for batch submission.
//!
//! Provides a thread-safe buffer that accumulates executed L2 blocks
//! and makes them available to the batch submission pipeline via the
//! [`BatchSource`] trait.

use std::{collections::VecDeque, sync::Arc};

use async_trait::async_trait;
use blocksource::OpBlock;
use channels::{BatchSource, L2BlockData, SourceError};
use tokio::sync::{RwLock, Semaphore};
use tracing::trace;

use crate::{convert::op_block_to_l2_data, errors::BufferError};

/// A thread-safe buffer that accumulates executed L2 blocks for batch submission.
///
/// The buffer receives blocks from the execution layer after they have been
/// verified, converts them to [`L2BlockData`], and provides them to the
/// batch submission pipeline via the [`BatchSource`] trait.
///
/// # Backpressure
///
/// The buffer has a configurable capacity. When full, calls to [`push`](Self::push)
/// will wait until space becomes available. Use [`try_push`](Self::try_push) for
/// non-blocking behavior.
///
/// # Thread Safety
///
/// The buffer is fully thread-safe and can be shared across multiple tasks
/// using `Arc<ExecutedBlockBuffer>`.
///
/// # Example
///
/// ```ignore
/// use sequencer::ExecutedBlockBuffer;
/// use channels::BatchSource;
///
/// let buffer = Arc::new(ExecutedBlockBuffer::new(256));
///
/// // Producer: push executed blocks
/// buffer.push(block).await?;
///
/// // Consumer: drain pending blocks for batching
/// let blocks = buffer.pending_blocks().await?;
/// ```
#[derive(Debug)]
pub struct ExecutedBlockBuffer {
    /// Queue of blocks ready for batching.
    pending: Arc<RwLock<VecDeque<L2BlockData>>>,
    /// Maximum buffer capacity.
    capacity: usize,
    /// Semaphore for backpressure (tracks available slots).
    slots: Arc<Semaphore>,
}

impl ExecutedBlockBuffer {
    /// Creates a new buffer with the specified capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of blocks the buffer can hold
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Buffer capacity must be greater than zero");

        Self {
            pending: Arc::new(RwLock::new(VecDeque::with_capacity(capacity))),
            capacity,
            slots: Arc::new(Semaphore::new(capacity)),
        }
    }

    /// Pushes an executed block to the buffer, waiting if full.
    ///
    /// Converts the block to [`L2BlockData`] and enqueues it. If the buffer
    /// is at capacity, this method will wait until space becomes available.
    ///
    /// # Arguments
    ///
    /// * `block` - The executed Optimism block to buffer
    ///
    /// # Errors
    ///
    /// Returns an error if the semaphore is closed (which shouldn't happen
    /// in normal operation).
    pub async fn push(&self, block: OpBlock) -> Result<(), BufferError> {
        // Wait for an available slot
        let permit = self.slots.acquire().await.map_err(|_| BufferError::LockFailed)?;

        // Convert and enqueue
        let l2_data = op_block_to_l2_data(&block);

        {
            let mut guard = self.pending.write().await;
            guard.push_back(l2_data);
        }

        // Forget the permit - it will be released when blocks are consumed
        permit.forget();

        trace!(
            capacity = self.capacity,
            available = self.slots.available_permits(),
            "Block pushed to buffer"
        );

        Ok(())
    }

    /// Attempts to push a block without waiting.
    ///
    /// Returns immediately with an error if the buffer is full.
    ///
    /// # Arguments
    ///
    /// * `block` - The executed Optimism block to buffer
    ///
    /// # Errors
    ///
    /// Returns [`BufferError::Full`] if the buffer is at capacity.
    pub fn try_push(&self, block: OpBlock) -> Result<(), BufferError> {
        // Try to acquire a slot without waiting
        let permit =
            self.slots.try_acquire().map_err(|_| BufferError::Full { capacity: self.capacity })?;

        // Convert and enqueue
        let l2_data = op_block_to_l2_data(&block);

        // Use try_write to avoid blocking
        let mut guard = self.pending.try_write().map_err(|_| BufferError::LockFailed)?;
        guard.push_back(l2_data);

        // Forget the permit
        permit.forget();

        Ok(())
    }

    /// Returns the number of blocks currently in the buffer.
    pub async fn len(&self) -> usize {
        self.pending.read().await.len()
    }

    /// Returns true if the buffer is empty.
    pub async fn is_empty(&self) -> bool {
        self.pending.read().await.is_empty()
    }

    /// Returns true if the buffer is at capacity.
    pub fn is_full(&self) -> bool {
        self.slots.available_permits() == 0
    }

    /// Returns the buffer's capacity.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the number of available slots.
    pub fn available(&self) -> usize {
        self.slots.available_permits()
    }
}

#[async_trait]
impl BatchSource for ExecutedBlockBuffer {
    /// Drains all pending blocks from the buffer.
    ///
    /// Returns all currently buffered blocks and releases their slots,
    /// allowing more blocks to be pushed.
    async fn pending_blocks(&mut self) -> Result<Vec<L2BlockData>, SourceError> {
        let blocks: Vec<L2BlockData> = {
            let mut guard = self.pending.write().await;
            guard.drain(..).collect()
        };

        // Release the slots for the drained blocks
        self.slots.add_permits(blocks.len());

        trace!(
            drained = blocks.len(),
            available = self.slots.available_permits(),
            "Blocks drained from buffer"
        );

        Ok(blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "Buffer capacity must be greater than zero")]
    fn new_panics_on_zero_capacity() {
        let _ = ExecutedBlockBuffer::new(0);
    }

    #[test]
    fn new_creates_empty_buffer() {
        let buffer = ExecutedBlockBuffer::new(10);
        assert_eq!(buffer.capacity(), 10);
        assert_eq!(buffer.available(), 10);
        assert!(!buffer.is_full());
    }

    #[tokio::test]
    async fn len_returns_zero_for_empty_buffer() {
        let buffer = ExecutedBlockBuffer::new(10);
        assert_eq!(buffer.len().await, 0);
        assert!(buffer.is_empty().await);
    }

    #[test]
    fn is_full_when_no_permits() {
        let buffer = ExecutedBlockBuffer::new(1);
        // Initially not full
        assert!(!buffer.is_full());
    }

    #[test]
    fn capacity_returns_configured_value() {
        let buffer = ExecutedBlockBuffer::new(42);
        assert_eq!(buffer.capacity(), 42);
    }

    #[test]
    fn available_returns_permit_count() {
        let buffer = ExecutedBlockBuffer::new(100);
        assert_eq!(buffer.available(), 100);
    }
}
