//! Builder pattern for constructing a fully-configured Node.
//!
//! The NodeBuilder provides a fluent API for configuring all aspects of a node
//! before construction. It validates the configuration at build time to ensure
//! that all required components are present based on the node's role.

use std::{
    fmt::Debug,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use blocksource::BlockProducer;
use montana_checkpoint::Checkpoint;
use montana_roles::Role;
use tokio::sync::{broadcast, mpsc};

use crate::{
    NodeConfig, NodeEvent, NodeRole, NodeState, SyncStage, SyncStatus, observer::NodeObserver,
};

/// Builder for constructing a fully-configured Node.
///
/// The builder validates that:
/// - Sequencer roles have a sequencer component
/// - Validator roles have a validator component
/// - Dual roles have both components
/// - Non-skipped sync requires a sync stage
///
/// # Examples
///
/// ```ignore
/// use montana_node::{NodeBuilder, NodeConfig, NodeRole};
/// use std::path::PathBuf;
///
/// # async fn example() -> eyre::Result<()> {
/// let config = NodeConfig {
///     role: NodeRole::Sequencer,
///     checkpoint_path: PathBuf::from("./data/checkpoint.json"),
///     checkpoint_interval_secs: 10,
///     skip_sync: false,
/// };
///
/// let node = NodeBuilder::new()
///     .with_config(config)
///     .with_sequencer(my_sequencer)
///     .with_sync_stage(sync_stage)
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub struct NodeBuilder<P: BlockProducer> {
    /// Node configuration
    config: NodeConfig,
    /// Optional sync stage
    sync_stage: Option<SyncStage<P>>,
    /// Optional sequencer component
    sequencer: Option<Box<dyn Role>>,
    /// Optional validator component
    validator: Option<Box<dyn Role>>,
    /// Observers for node events
    observers: Vec<Arc<dyn NodeObserver>>,
}

impl<P: BlockProducer + Debug> Debug for NodeBuilder<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeBuilder")
            .field("config", &self.config)
            .field("sync_stage", &self.sync_stage)
            .field("sequencer", &self.sequencer.as_ref().map(|s| s.name()))
            .field("validator", &self.validator.as_ref().map(|v| v.name()))
            .field("observers", &self.observers.len())
            .finish()
    }
}

impl<P: BlockProducer + 'static> NodeBuilder<P> {
    /// Create a new NodeBuilder with default configuration.
    pub fn new() -> Self {
        Self {
            config: NodeConfig::default(),
            sync_stage: None,
            sequencer: None,
            validator: None,
            observers: Vec::new(),
        }
    }

    /// Set the node configuration.
    ///
    /// This replaces any previous configuration with the provided one.
    pub fn with_config(mut self, config: NodeConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the node role.
    ///
    /// This is a convenience method that updates the role in the configuration.
    pub const fn with_role(mut self, role: NodeRole) -> Self {
        self.config.role = role;
        self
    }

    /// Set the checkpoint path.
    ///
    /// This is a convenience method that updates the checkpoint path in the configuration.
    pub fn with_checkpoint_path(mut self, path: PathBuf) -> Self {
        self.config.checkpoint_path = path;
        self
    }

    /// Set the sync stage.
    ///
    /// The sync stage is responsible for catching up to the chain tip before
    /// the node becomes active. This is required unless sync is explicitly skipped.
    pub fn with_sync_stage(mut self, stage: SyncStage<P>) -> Self {
        self.sync_stage = Some(stage);
        self
    }

    /// Skip the sync stage.
    ///
    /// When set, the node will start from its current state without syncing
    /// to the chain tip. Use this when resuming from a recent checkpoint or
    /// when the node is already synced.
    pub const fn skip_sync(mut self) -> Self {
        self.config.skip_sync = true;
        self
    }

    /// Set the sequencer component.
    ///
    /// Required if the node role is Sequencer or Dual.
    pub fn with_sequencer<R: Role + 'static>(mut self, sequencer: R) -> Self {
        self.sequencer = Some(Box::new(sequencer));
        self
    }

    /// Set the validator component.
    ///
    /// Required if the node role is Validator or Dual.
    pub fn with_validator<R: Role + 'static>(mut self, validator: R) -> Self {
        self.validator = Some(Box::new(validator));
        self
    }

    /// Add an observer for node events.
    ///
    /// Observers are called synchronously when events occur. Multiple
    /// observers can be registered and will be called in order.
    pub fn with_observer(mut self, observer: Arc<dyn NodeObserver>) -> Self {
        self.observers.push(observer);
        self
    }

    /// Build the node, consuming the builder.
    ///
    /// This validates the configuration and constructs a fully-configured Node.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The role requires a sequencer but none was provided
    /// - The role requires a validator but none was provided
    /// - Sync is not skipped but no sync stage was provided
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use montana_node::{NodeBuilder, NodeRole};
    /// # async fn example() -> eyre::Result<()> {
    /// let node = NodeBuilder::new()
    ///     .with_role(NodeRole::Sequencer)
    ///     .with_sequencer(my_sequencer)
    ///     .with_sync_stage(sync_stage)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(self) -> eyre::Result<Node<P>> {
        // Validate role-specific requirements
        if self.config.role.runs_sequencer() && self.sequencer.is_none() {
            eyre::bail!(
                "Node role {:?} requires a sequencer, but none was provided. \
                 Use with_sequencer() to provide one.",
                self.config.role
            );
        }

        if self.config.role.runs_validator() && self.validator.is_none() {
            eyre::bail!(
                "Node role {:?} requires a validator, but none was provided. \
                 Use with_validator() to provide one.",
                self.config.role
            );
        }

        // Validate sync stage requirements
        if !self.config.skip_sync && self.sync_stage.is_none() {
            eyre::bail!(
                "Sync is not skipped but no sync stage was provided. \
                 Use with_sync_stage() to provide one or skip_sync() to skip syncing."
            );
        }

        // Create broadcast channel for events
        let (event_tx, _) = broadcast::channel(1024);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        // Try to load checkpoint, or use default
        let checkpoint =
            Checkpoint::load(&self.config.checkpoint_path).ok().flatten().unwrap_or_else(|| {
                tracing::info!("No checkpoint found, starting fresh");
                Checkpoint::default()
            });

        Ok(Node {
            config: self.config,
            state: NodeState::Starting,
            checkpoint,
            sync_stage: self.sync_stage,
            sequencer: self.sequencer,
            validator: self.validator,
            event_tx,
            observers: self.observers,
            shutdown_rx,
            shutdown_tx,
            last_checkpoint_save: Instant::now(),
        })
    }
}

impl<P: BlockProducer + 'static> Default for NodeBuilder<P> {
    fn default() -> Self {
        Self::new()
    }
}

/// A fully-configured Montana node.
///
/// The node manages the lifecycle of blockchain operations including:
/// - Syncing to the chain tip
/// - Executing transactions (sequencer role)
/// - Validating blocks (validator role)
/// - Checkpointing state
/// - Event broadcasting to observers
/// - Graceful shutdown
///
/// Use [`NodeBuilder`] to construct a node with the appropriate configuration.
pub struct Node<P: BlockProducer> {
    /// Node configuration
    config: NodeConfig,
    /// Current node state
    state: NodeState,
    /// Checkpoint for persistence
    checkpoint: Checkpoint,
    /// Optional sync stage for catching up to chain tip
    sync_stage: Option<SyncStage<P>>,
    /// Optional sequencer component
    sequencer: Option<Box<dyn Role>>,
    /// Optional validator component
    validator: Option<Box<dyn Role>>,
    /// Event broadcaster for observers
    event_tx: broadcast::Sender<NodeEvent>,
    /// Observers for synchronous event notification
    observers: Vec<Arc<dyn NodeObserver>>,
    /// Shutdown signal receiver
    shutdown_rx: mpsc::Receiver<()>,
    /// Shutdown signal sender (for external shutdown requests)
    shutdown_tx: mpsc::Sender<()>,
    /// Last time checkpoint was saved
    last_checkpoint_save: Instant,
}

impl<P: BlockProducer + Debug> Debug for Node<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("config", &self.config)
            .field("state", &self.state)
            .field("checkpoint", &self.checkpoint)
            .field("sync_stage", &self.sync_stage)
            .field("sequencer", &self.sequencer.as_ref().map(|s| s.name()))
            .field("validator", &self.validator.as_ref().map(|v| v.name()))
            .field("last_checkpoint_save", &self.last_checkpoint_save)
            .finish_non_exhaustive()
    }
}

impl<P: BlockProducer> Node<P> {
    /// Create a new builder for constructing a node.
    pub fn builder() -> NodeBuilder<P>
    where
        P: 'static,
    {
        NodeBuilder::new()
    }

    /// Get a reference to the node's configuration.
    pub const fn config(&self) -> &NodeConfig {
        &self.config
    }

    /// Get a mutable reference to the sync stage, if present.
    pub const fn sync_stage_mut(&mut self) -> Option<&mut SyncStage<P>> {
        self.sync_stage.as_mut()
    }

    /// Get a reference to the sync stage, if present.
    pub const fn sync_stage(&self) -> Option<&SyncStage<P>> {
        self.sync_stage.as_ref()
    }

    /// Get a mutable reference to the sequencer, if present.
    pub fn sequencer_mut(&mut self) -> Option<&mut Box<dyn Role>> {
        self.sequencer.as_mut()
    }

    /// Get a reference to the sequencer, if present.
    pub fn sequencer(&self) -> Option<&dyn Role> {
        self.sequencer.as_deref()
    }

    /// Get a mutable reference to the validator, if present.
    pub fn validator_mut(&mut self) -> Option<&mut Box<dyn Role>> {
        self.validator.as_mut()
    }

    /// Get a reference to the validator, if present.
    pub fn validator(&self) -> Option<&dyn Role> {
        self.validator.as_deref()
    }

    /// Subscribes to node events.
    ///
    /// Returns a receiver that will receive all node events. Multiple subscribers
    /// are supported via the broadcast channel.
    pub fn subscribe(&self) -> broadcast::Receiver<NodeEvent> {
        self.event_tx.subscribe()
    }

    /// Gets a handle for requesting shutdown.
    ///
    /// Send a message to this channel to request the node to shutdown gracefully.
    pub fn shutdown_handle(&self) -> mpsc::Sender<()> {
        self.shutdown_tx.clone()
    }

    /// Add an observer to receive events.
    ///
    /// Observers are called synchronously for each event. This method
    /// allows adding observers after node construction.
    pub fn add_observer(&mut self, observer: Arc<dyn NodeObserver>) {
        self.observers.push(observer);
    }

    /// Gets the current node state.
    pub const fn state(&self) -> &NodeState {
        &self.state
    }

    /// Gets the current checkpoint.
    pub const fn checkpoint(&self) -> &Checkpoint {
        &self.checkpoint
    }

    /// Main entry point for running the node.
    ///
    /// This is the primary loop that:
    /// 1. Runs the sync stage if needed
    /// 2. Transitions to active stage
    /// 3. Runs configured roles
    /// 4. Handles shutdown signals
    ///
    /// # Errors
    /// Returns an error if any critical operation fails.
    pub async fn run(&mut self) -> eyre::Result<()> {
        tracing::info!("Node starting with role: {:?}", self.config.role);

        // Run sync stage if not skipped
        if !self.config.skip_sync {
            self.run_sync_stage().await?;
        } else {
            tracing::info!("Skipping sync stage as configured");
        }

        // Transition to active stage
        self.set_state(NodeState::Active).await;

        // Resume roles from checkpoint
        if let Some(ref mut sequencer) = self.sequencer {
            sequencer.resume(&self.checkpoint).await?;
        }
        if let Some(ref mut validator) = self.validator {
            validator.resume(&self.checkpoint).await?;
        }

        // Run active stage
        self.run_active_stage().await?;

        // Shutdown
        self.set_state(NodeState::Stopping).await;
        self.save_checkpoint().await?;
        self.set_state(NodeState::Stopped).await;

        tracing::info!("Node stopped");
        Ok(())
    }

    // Private helper methods

    /// Runs the sync stage.
    async fn run_sync_stage(&mut self) -> eyre::Result<()> {
        // Initialize sync from checkpoint
        let start_block = self.checkpoint.synced_to_block;

        if let Some(sync_stage) = self.sync_stage.as_mut() {
            sync_stage.initialize(start_block).await?;
            let progress = sync_stage.progress();

            // Emit SyncStarted event
            self.emit(NodeEvent::SyncStarted {
                start_block: progress.start_block,
                target_block: progress.target_block,
            })
            .await;

            self.set_state(NodeState::Syncing(progress)).await;
        } else {
            tracing::warn!("Sync stage not configured, skipping");
            return Ok(());
        }

        // Run sync loop
        loop {
            // Check for shutdown signal
            if self.shutdown_rx.try_recv().is_ok() {
                tracing::info!("Shutdown requested during sync");
                return Ok(());
            }

            // Tick sync stage
            let status = if let Some(sync_stage) = self.sync_stage.as_mut() {
                sync_stage.tick().await?
            } else {
                break;
            };

            match status {
                SyncStatus::Syncing(progress) => {
                    self.set_state(NodeState::Syncing(progress.clone())).await;

                    // Emit sync progress event
                    self.emit(NodeEvent::SyncProgress(progress.clone())).await;

                    // Maybe save checkpoint
                    self.maybe_save_checkpoint().await?;

                    // Update checkpoint from sync progress
                    self.checkpoint.record_sync_progress(progress.current_block);
                }
                SyncStatus::Complete => {
                    tracing::info!("Sync stage complete");

                    // Calculate sync stats
                    let blocks_synced = self.sync_stage.as_ref().map_or(0, |sync_stage| {
                        let progress = sync_stage.progress();
                        progress.current_block.saturating_sub(progress.start_block)
                    });

                    // Emit SyncCompleted event
                    self.emit(NodeEvent::SyncCompleted { blocks_synced, duration_secs: 0.0 }).await;

                    // Save final checkpoint
                    if let Some(sync_stage) = self.sync_stage.as_ref() {
                        self.checkpoint = sync_stage.checkpoint();
                    }
                    self.save_checkpoint().await?;
                    break;
                }
            }
        }

        Ok(())
    }

    /// Runs the active stage.
    async fn run_active_stage(&mut self) -> eyre::Result<()> {
        tracing::info!("Node active, running roles");

        loop {
            // Check for shutdown signal
            if self.shutdown_rx.try_recv().is_ok() {
                tracing::info!("Shutdown requested");
                break;
            }

            let mut progress_made = false;

            // Tick sequencer if configured
            if let Some(ref mut sequencer) = self.sequencer {
                match sequencer.tick().await? {
                    montana_roles::TickResult::Progress => {
                        progress_made = true;
                    }
                    montana_roles::TickResult::Idle => {}
                    montana_roles::TickResult::Complete => {
                        tracing::info!("Sequencer completed");
                    }
                }

                // Update checkpoint from sequencer
                let role_cp = sequencer.checkpoint();
                if let Some(batch) = role_cp.last_batch_submitted {
                    self.checkpoint.record_batch_submitted(batch);
                }
                if let Some(block) = role_cp.last_block_processed {
                    self.checkpoint.record_block_executed(block);
                }
            }

            // Tick validator if configured
            if let Some(ref mut validator) = self.validator {
                match validator.tick().await? {
                    montana_roles::TickResult::Progress => {
                        progress_made = true;
                    }
                    montana_roles::TickResult::Idle => {}
                    montana_roles::TickResult::Complete => {
                        tracing::info!("Validator completed");
                    }
                }

                // Update checkpoint from validator
                let role_cp = validator.checkpoint();
                if let Some(batch) = role_cp.last_batch_derived {
                    self.checkpoint.record_batch_derived(batch);
                }
                if let Some(block) = role_cp.last_block_processed {
                    self.checkpoint.record_block_executed(block);
                }
            }

            // Maybe save checkpoint
            self.maybe_save_checkpoint().await?;

            // If no progress was made, sleep briefly to avoid spinning
            if !progress_made {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        Ok(())
    }

    /// Emits an event to all subscribers.
    async fn emit(&self, event: NodeEvent) {
        // Notify synchronous observers first
        for observer in &self.observers {
            observer.on_event(&event);
        }
        // Then send to broadcast channel for async subscribers
        let _ = self.event_tx.send(event);
    }

    /// Sets the node state and emits a state change event.
    async fn set_state(&mut self, new_state: NodeState) {
        self.state = new_state.clone();
        self.emit(NodeEvent::StateChanged(new_state)).await;
    }

    /// Saves the checkpoint if enough time has passed since the last save.
    async fn maybe_save_checkpoint(&mut self) -> eyre::Result<()> {
        let interval = Duration::from_secs(self.config.checkpoint_interval_secs);

        if self.last_checkpoint_save.elapsed() >= interval {
            self.save_checkpoint().await?;
        }

        Ok(())
    }

    /// Saves the checkpoint to disk.
    async fn save_checkpoint(&mut self) -> eyre::Result<()> {
        self.checkpoint.touch();
        self.checkpoint.save(&self.config.checkpoint_path)?;
        self.last_checkpoint_save = Instant::now();

        tracing::debug!("Checkpoint saved to {:?}", self.config.checkpoint_path);

        self.emit(NodeEvent::CheckpointSaved(self.checkpoint.clone())).await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock BlockProducer for testing
    struct MockBlockProducer;

    #[async_trait::async_trait]
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

    // Mock Role for testing
    struct MockSequencer;

    #[async_trait::async_trait]
    impl Role for MockSequencer {
        fn name(&self) -> &'static str {
            "mock_sequencer"
        }

        async fn resume(
            &mut self,
            _checkpoint: &montana_checkpoint::Checkpoint,
        ) -> eyre::Result<()> {
            Ok(())
        }

        async fn tick(&mut self) -> eyre::Result<montana_roles::TickResult> {
            Ok(montana_roles::TickResult::Idle)
        }

        fn checkpoint(&self) -> montana_roles::RoleCheckpoint {
            montana_roles::RoleCheckpoint::default()
        }
    }

    struct MockValidator;

    #[async_trait::async_trait]
    impl Role for MockValidator {
        fn name(&self) -> &'static str {
            "mock_validator"
        }

        async fn resume(
            &mut self,
            _checkpoint: &montana_checkpoint::Checkpoint,
        ) -> eyre::Result<()> {
            Ok(())
        }

        async fn tick(&mut self) -> eyre::Result<montana_roles::TickResult> {
            Ok(montana_roles::TickResult::Idle)
        }

        fn checkpoint(&self) -> montana_roles::RoleCheckpoint {
            montana_roles::RoleCheckpoint::default()
        }
    }

    #[test]
    fn test_builder_new() {
        let builder = NodeBuilder::<MockBlockProducer>::new();
        assert_eq!(builder.config, NodeConfig::default());
        assert!(builder.sync_stage.is_none());
        assert!(builder.sequencer.is_none());
        assert!(builder.validator.is_none());
    }

    #[test]
    fn test_builder_with_config() {
        let config = NodeConfig {
            role: NodeRole::Sequencer,
            checkpoint_path: PathBuf::from("/tmp/checkpoint"),
            checkpoint_interval_secs: 20,
            skip_sync: true,
        };
        let builder = NodeBuilder::<MockBlockProducer>::new().with_config(config.clone());
        assert_eq!(builder.config, config);
    }

    #[test]
    fn test_builder_with_role() {
        let builder = NodeBuilder::<MockBlockProducer>::new().with_role(NodeRole::Validator);
        assert_eq!(builder.config.role, NodeRole::Validator);
    }

    #[test]
    fn test_builder_with_checkpoint_path() {
        let path = PathBuf::from("/custom/checkpoint.json");
        let builder = NodeBuilder::<MockBlockProducer>::new().with_checkpoint_path(path.clone());
        assert_eq!(builder.config.checkpoint_path, path);
    }

    #[test]
    fn test_builder_skip_sync() {
        let builder = NodeBuilder::<MockBlockProducer>::new().skip_sync();
        assert!(builder.config.skip_sync);
    }

    #[test]
    fn test_builder_sequencer_role_requires_sequencer() {
        let result = NodeBuilder::<MockBlockProducer>::new()
            .with_role(NodeRole::Sequencer)
            .skip_sync()
            .build();

        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("requires a sequencer"));
    }

    #[test]
    fn test_builder_validator_role_requires_validator() {
        let result = NodeBuilder::<MockBlockProducer>::new()
            .with_role(NodeRole::Validator)
            .skip_sync()
            .build();

        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("requires a validator"));
    }

    #[test]
    fn test_builder_dual_role_requires_both() {
        // Missing both
        let result =
            NodeBuilder::<MockBlockProducer>::new().with_role(NodeRole::Dual).skip_sync().build();
        assert!(result.is_err());

        // Missing validator
        let result = NodeBuilder::<MockBlockProducer>::new()
            .with_role(NodeRole::Dual)
            .with_sequencer(MockSequencer)
            .skip_sync()
            .build();
        assert!(result.is_err());

        // Missing sequencer
        let result = NodeBuilder::<MockBlockProducer>::new()
            .with_role(NodeRole::Dual)
            .with_validator(MockValidator)
            .skip_sync()
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_requires_sync_stage_unless_skipped() {
        let result = NodeBuilder::<MockBlockProducer>::new()
            .with_role(NodeRole::Sequencer)
            .with_sequencer(MockSequencer)
            .build();

        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("no sync stage was provided"));
    }

    #[test]
    fn test_builder_success_with_skip_sync() {
        let result = NodeBuilder::<MockBlockProducer>::new()
            .with_role(NodeRole::Sequencer)
            .with_sequencer(MockSequencer)
            .skip_sync()
            .build();

        assert!(result.is_ok());
        let node = result.unwrap();
        assert_eq!(node.config.role, NodeRole::Sequencer);
        assert!(node.sequencer.is_some());
        assert!(node.validator.is_none());
        assert!(node.sync_stage.is_none());
    }

    #[test]
    fn test_builder_success_with_sync_stage() {
        use crate::SyncConfig;

        let sync_stage = SyncStage::new(MockBlockProducer, SyncConfig::default());
        let result = NodeBuilder::<MockBlockProducer>::new()
            .with_role(NodeRole::Validator)
            .with_validator(MockValidator)
            .with_sync_stage(sync_stage)
            .build();

        assert!(result.is_ok());
        let node = result.unwrap();
        assert_eq!(node.config.role, NodeRole::Validator);
        assert!(node.validator.is_some());
        assert!(node.sequencer.is_none());
        assert!(node.sync_stage.is_some());
    }

    #[test]
    fn test_builder_dual_role_success() {
        let result = NodeBuilder::<MockBlockProducer>::new()
            .with_role(NodeRole::Dual)
            .with_sequencer(MockSequencer)
            .with_validator(MockValidator)
            .skip_sync()
            .build();

        assert!(result.is_ok());
        let node = result.unwrap();
        assert_eq!(node.config.role, NodeRole::Dual);
        assert!(node.sequencer.is_some());
        assert!(node.validator.is_some());
    }

    #[test]
    fn test_node_accessors() {
        let node = NodeBuilder::<MockBlockProducer>::new()
            .with_role(NodeRole::Dual)
            .with_sequencer(MockSequencer)
            .with_validator(MockValidator)
            .skip_sync()
            .build()
            .unwrap();

        assert_eq!(node.config().role, NodeRole::Dual);
        assert!(node.sequencer().is_some());
        assert!(node.validator().is_some());
        assert!(node.sync_stage().is_none());
    }

    #[test]
    fn test_node_mutable_accessors() {
        let mut node = NodeBuilder::<MockBlockProducer>::new()
            .with_role(NodeRole::Dual)
            .with_sequencer(MockSequencer)
            .with_validator(MockValidator)
            .skip_sync()
            .build()
            .unwrap();

        assert!(node.sequencer_mut().is_some());
        assert!(node.validator_mut().is_some());
        assert!(node.sync_stage_mut().is_none());
    }

    #[test]
    fn test_default_builder() {
        let builder1 = NodeBuilder::<MockBlockProducer>::new();
        let builder2 = NodeBuilder::<MockBlockProducer>::default();
        assert_eq!(builder1.config, builder2.config);
    }

    #[test]
    fn test_node_builder_method() {
        let builder = Node::<MockBlockProducer>::builder();
        assert_eq!(builder.config, NodeConfig::default());
    }

    #[test]
    fn test_observer_dispatch() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        use crate::observer::NodeObserver;

        struct CountingObserver {
            count: AtomicUsize,
        }

        impl NodeObserver for CountingObserver {
            fn on_event(&self, _event: &NodeEvent) {
                self.count.fetch_add(1, Ordering::SeqCst);
            }
        }

        let observer = Arc::new(CountingObserver { count: AtomicUsize::new(0) });
        let node = NodeBuilder::<MockBlockProducer>::new()
            .with_role(NodeRole::Sequencer)
            .with_sequencer(MockSequencer)
            .with_observer(observer.clone())
            .skip_sync()
            .build()
            .unwrap();

        // Observers should be registered
        assert_eq!(node.observers.len(), 1);

        // Manually test observer is called
        node.observers[0].on_event(&NodeEvent::StateChanged(NodeState::Starting));
        assert_eq!(observer.count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_add_observer_after_build() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        use crate::observer::NodeObserver;

        struct CountingObserver {
            count: AtomicUsize,
        }

        impl NodeObserver for CountingObserver {
            fn on_event(&self, _event: &NodeEvent) {
                self.count.fetch_add(1, Ordering::SeqCst);
            }
        }

        let mut node = NodeBuilder::<MockBlockProducer>::new()
            .with_role(NodeRole::Sequencer)
            .with_sequencer(MockSequencer)
            .skip_sync()
            .build()
            .unwrap();

        assert_eq!(node.observers.len(), 0);

        let observer = Arc::new(CountingObserver { count: AtomicUsize::new(0) });
        node.add_observer(observer.clone());

        assert_eq!(node.observers.len(), 1);
    }
}
