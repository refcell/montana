//! Sync stage implementation for catching up to chain tip.
//!
//! The sync stage is responsible for catching up to the current blockchain state
//! before the node becomes active. It fetches and processes blocks from a block
//! producer until the node is within a threshold of the chain tip.

use std::time::{Duration, Instant};

use blocksource::BlockProducer;
use montana_checkpoint::Checkpoint;
use tokio::sync::mpsc;

use crate::SyncProgress;

/// Configuration for the sync stage.
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// How many blocks behind tip to consider "synced".
    /// This prevents thrashing between synced/syncing states when near tip.
    pub sync_threshold: u64,
    /// How often to refresh the target block (chain tip).
    pub tip_refresh_interval: Duration,
    /// Maximum blocks to process per tick.
    pub blocks_per_tick: usize,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            sync_threshold: 10,
            tip_refresh_interval: Duration::from_secs(12),
            blocks_per_tick: 100,
        }
    }
}

/// Events emitted by the sync stage for observer pattern.
#[derive(Debug, Clone)]
pub enum SyncEvent {
    /// Sync started with initial parameters.
    Started {
        /// Block number where sync started
        start_block: u64,
        /// Target block to sync to
        target_block: u64,
    },
    /// Progress update during sync.
    Progress {
        /// Current block being processed
        current_block: u64,
        /// Target block to sync to
        target_block: u64,
        /// Blocks synced per second
        blocks_per_second: f64,
    },
    /// Sync completed successfully.
    Completed {
        /// Total blocks synced
        blocks_synced: u64,
        /// Total duration of sync
        duration: Duration,
        /// Average blocks per second
        blocks_per_second: f64,
    },
}

/// Status of the sync stage.
#[derive(Debug, Clone)]
pub enum SyncStatus {
    /// Currently syncing with progress information
    Syncing(SyncProgress),
    /// Sync complete, ready for active operation
    Complete,
}

/// The sync stage: catches up to chain tip before active operation.
///
/// This stage is responsible for:
/// - Fetching blocks from the block producer
/// - Processing blocks until within sync threshold of chain tip
/// - Emitting events for progress tracking
/// - Periodically refreshing the chain tip
#[derive(Debug)]
pub struct SyncStage<P: BlockProducer> {
    /// Block producer for fetching blocks
    block_producer: P,
    /// Configuration
    config: SyncConfig,
    /// Block number at which syncing started
    start_block: u64,
    /// Current block being processed
    current_block: u64,
    /// Target block (chain tip)
    target_block: u64,
    /// Last time we refreshed the tip
    last_tip_refresh: Instant,
    /// Total blocks synced
    blocks_synced: u64,
    /// Time when sync started
    sync_start_time: Option<Instant>,
    /// Channel to send events to observers
    event_tx: Option<mpsc::UnboundedSender<SyncEvent>>,
}

impl<P: BlockProducer> SyncStage<P> {
    /// Create a new sync stage with the given block producer and configuration.
    pub fn new(block_producer: P, config: SyncConfig) -> Self {
        Self {
            block_producer,
            config,
            start_block: 0,
            current_block: 0,
            target_block: 0,
            last_tip_refresh: Instant::now(),
            blocks_synced: 0,
            sync_start_time: None,
            event_tx: None,
        }
    }

    /// Set the event sender for emitting sync events.
    pub fn with_event_sender(mut self, tx: mpsc::UnboundedSender<SyncEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Initialize sync from a checkpoint.
    ///
    /// This fetches the current chain tip and sets up the initial sync state.
    ///
    /// # Errors
    /// Returns an error if the chain tip cannot be fetched.
    pub async fn initialize(&mut self, start_block: u64) -> eyre::Result<()> {
        self.start_block = start_block;
        self.current_block = start_block;

        // Fetch current chain tip
        self.refresh_tip().await?;

        tracing::info!(
            start = self.start_block,
            target = self.target_block,
            "Sync stage initialized, {} blocks to sync",
            self.blocks_remaining()
        );

        // Emit started event
        self.emit(SyncEvent::Started {
            start_block: self.start_block,
            target_block: self.target_block,
        });

        Ok(())
    }

    /// Refresh the target block (chain tip).
    ///
    /// # Errors
    /// Returns an error if the chain tip cannot be fetched.
    async fn refresh_tip(&mut self) -> eyre::Result<()> {
        let tip = self.block_producer.get_chain_tip().await?;
        self.target_block = tip;
        self.last_tip_refresh = Instant::now();
        Ok(())
    }

    /// Check if we should refresh the tip based on the configured interval.
    fn should_refresh_tip(&self) -> bool {
        self.last_tip_refresh.elapsed() >= self.config.tip_refresh_interval
    }

    /// Check if sync is complete.
    ///
    /// Returns true if the current block is within the sync threshold of the target.
    pub fn is_synced(&self) -> bool {
        self.current_block + self.config.sync_threshold >= self.target_block
    }

    /// Run one tick of the sync stage.
    ///
    /// Processes up to `blocks_per_tick` blocks, refreshing the tip if needed.
    ///
    /// # Errors
    /// Returns an error if block fetching or processing fails.
    pub async fn tick(&mut self) -> eyre::Result<SyncStatus> {
        // Start timer on first tick
        if self.sync_start_time.is_none() {
            self.sync_start_time = Some(Instant::now());
        }

        // Refresh tip periodically
        if self.should_refresh_tip() {
            self.refresh_tip().await?;
        }

        // Check if already synced
        if self.is_synced() {
            // Emit completion event
            if let Some(start) = self.sync_start_time {
                let duration = start.elapsed();
                let blocks_per_second = if duration.as_secs_f64() > 0.0 {
                    self.blocks_synced as f64 / duration.as_secs_f64()
                } else {
                    0.0
                };

                self.emit(SyncEvent::Completed {
                    blocks_synced: self.blocks_synced,
                    duration,
                    blocks_per_second,
                });
            }

            return Ok(SyncStatus::Complete);
        }

        // Process blocks up to blocks_per_tick
        let mut processed = 0;
        while processed < self.config.blocks_per_tick && !self.is_synced() {
            let next_block = self.current_block + 1;

            // Fetch block
            let _block = match self.block_producer.get_block(next_block).await? {
                Some(b) => b,
                None => break, // No more blocks available
            };

            // TODO: Execute block and commit to database
            // For now, just update the current block
            self.current_block = next_block;
            self.blocks_synced += 1;
            processed += 1;
        }

        // Calculate progress and emit event
        let progress = self.progress();
        self.emit(SyncEvent::Progress {
            current_block: progress.current_block,
            target_block: progress.target_block,
            blocks_per_second: progress.blocks_per_second,
        });

        Ok(SyncStatus::Syncing(progress))
    }

    /// Get the current sync progress.
    pub fn progress(&self) -> SyncProgress {
        let blocks_per_second = if let Some(start) = self.sync_start_time {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 { self.blocks_synced as f64 / elapsed } else { 0.0 }
        } else {
            0.0
        };

        let eta = if blocks_per_second > 0.0 {
            let remaining = self.blocks_remaining();
            let eta_secs = remaining as f64 / blocks_per_second;
            Some(Duration::from_secs_f64(eta_secs))
        } else {
            None
        };

        SyncProgress {
            start_block: self.start_block,
            current_block: self.current_block,
            target_block: self.target_block,
            blocks_per_second,
            eta,
        }
    }

    /// Get the current checkpoint for the sync state.
    pub fn checkpoint(&self) -> Checkpoint {
        let mut cp = Checkpoint::default();
        cp.synced_to_block = self.current_block;
        cp.last_block_executed = self.current_block;
        cp.touch();
        cp
    }

    /// Emit an event to observers.
    fn emit(&self, event: SyncEvent) {
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Get the number of blocks remaining to sync.
    fn blocks_remaining(&self) -> u64 {
        self.target_block.saturating_sub(self.current_block)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;

    #[test]
    fn test_sync_config_defaults() {
        let config = SyncConfig::default();
        assert_eq!(config.sync_threshold, 10);
        assert_eq!(config.tip_refresh_interval, Duration::from_secs(12));
        assert_eq!(config.blocks_per_tick, 100);
    }

    #[test]
    fn test_sync_progress_calculation() {
        let progress = SyncProgress {
            start_block: 100,
            current_block: 150,
            target_block: 200,
            blocks_per_second: 10.0,
            eta: Some(Duration::from_secs(5)),
        };

        assert_eq!(progress.progress(), 0.5); // 50% complete
        assert_eq!(progress.blocks_remaining(), 50);
        assert!(!progress.is_complete());
    }

    #[test]
    fn test_sync_progress_complete() {
        let progress = SyncProgress {
            start_block: 100,
            current_block: 200,
            target_block: 200,
            blocks_per_second: 10.0,
            eta: None,
        };

        assert_eq!(progress.progress(), 1.0); // 100% complete
        assert_eq!(progress.blocks_remaining(), 0);
        assert!(progress.is_complete());
    }

    #[test]
    fn test_sync_progress_edge_case() {
        // Target equals start
        let progress = SyncProgress {
            start_block: 100,
            current_block: 100,
            target_block: 100,
            blocks_per_second: 0.0,
            eta: None,
        };

        assert_eq!(progress.progress(), 1.0);
        assert_eq!(progress.blocks_remaining(), 0);
        assert!(progress.is_complete());
    }

    #[test]
    fn test_is_synced_logic() {
        let config = SyncConfig {
            sync_threshold: 10,
            tip_refresh_interval: Duration::from_secs(12),
            blocks_per_tick: 100,
        };

        let mut stage = SyncStage {
            block_producer: MockBlockProducer,
            config,
            start_block: 0,
            current_block: 90,
            target_block: 100,
            last_tip_refresh: Instant::now(),
            blocks_synced: 0,
            sync_start_time: None,
            event_tx: None,
        };

        // Within threshold
        assert!(stage.is_synced());

        // Outside threshold
        stage.current_block = 89;
        assert!(!stage.is_synced());

        // Exactly at threshold
        stage.current_block = 90;
        assert!(stage.is_synced());
    }

    #[test]
    fn test_should_refresh_tip() {
        let config = SyncConfig {
            sync_threshold: 10,
            tip_refresh_interval: Duration::from_millis(100),
            blocks_per_tick: 100,
        };

        let stage = SyncStage {
            block_producer: MockBlockProducer,
            config,
            start_block: 0,
            current_block: 0,
            target_block: 100,
            last_tip_refresh: Instant::now(),
            blocks_synced: 0,
            sync_start_time: None,
            event_tx: None,
        };

        // Should not refresh immediately
        assert!(!stage.should_refresh_tip());

        // Create stage with old refresh time
        let mut stage = stage;
        stage.last_tip_refresh = Instant::now() - Duration::from_secs(1);
        assert!(stage.should_refresh_tip());
    }

    // Mock BlockProducer for testing
    struct MockBlockProducer;

    #[async_trait]
    impl BlockProducer for MockBlockProducer {
        async fn produce(
            &self,
            _tx: tokio::sync::mpsc::Sender<blocksource::OpBlock>,
        ) -> eyre::Result<()> {
            Ok(())
        }

        async fn get_chain_tip(&self) -> eyre::Result<u64> {
            Ok(100)
        }

        async fn get_block(&self, _number: u64) -> eyre::Result<Option<blocksource::OpBlock>> {
            Ok(None)
        }
    }
}
