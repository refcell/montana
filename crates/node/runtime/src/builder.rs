use std::{path::PathBuf, sync::Arc, time::Duration};

use alloy::providers::{Provider, ProviderBuilder};
use chainspec::BASE_MAINNET;
use database::{CachedDatabase, RocksDbKvDatabase, TrieDatabase};
use eyre::Result;
use montana_adapters::{
    BatchSinkAdapter, BatchSourceAdapter, BlockProducerWrapper, BlockTxCountStore,
    TuiBatchCallback, TuiDerivationCallback, TuiExecutionCallback,
};
use montana_batch_context::BatchContext;
use montana_batcher::{Address, BatcherConfig};
use montana_block_feeder::run_block_feeder;
use montana_brotli::BrotliCompressor;
use montana_checkpoint::Checkpoint;
use montana_cli::MontanaCli;
use montana_harness::{AnvilState, DEFAULT_GENESIS_STATE};
use montana_node::{Node, NodeBuilder, NodeConfig, SyncConfig, SyncStage};
use montana_roles::{Sequencer, Validator};
use montana_tui::{TuiEvent, TuiHandle, TuiObserver};
use op_alloy::network::Optimism;
use primitives::OpBlock;
use tokio::sync::mpsc;
use vm::BlockExecutor;

/// Build the node with the appropriate block producer based on CLI args.
pub async fn build_node(
    cli: MontanaCli,
    tui_observer: Option<Arc<TuiObserver>>,
    tui_handle: Option<TuiHandle>,
    rpc_url: String,
) -> Result<Node<BlockProducerWrapper>> {
    // Build provider
    let provider = ProviderBuilder::new()
        .disable_recommended_fillers()
        .network::<Optimism>()
        .connect(&rpc_url)
        .await?;

    // Determine start block:
    // 1. Use explicit --start if provided
    // 2. In harness mode with no explicit start, start from block 0 (to sync the initial blocks)
    // 3. Otherwise, try loading from checkpoint file
    // 4. Fall back to chain tip if no checkpoint exists
    let start_block = if let Some(start) = cli.start {
        start
    } else if cli.with_harness {
        // In harness mode, always start from block 0 to sync through the initial blocks
        tracing::info!("Harness mode: starting sync from block 0");
        0
    } else if let Some(checkpoint) = Checkpoint::load(&cli.checkpoint_path)
        .map_err(|e| eyre::eyre!("Failed to load checkpoint: {}", e))?
    {
        // Resume from checkpoint: use synced_to_block + 1 (next block to process)
        let resume_block = checkpoint.synced_to_block.saturating_add(1);
        tracing::info!(
            checkpoint_block = checkpoint.synced_to_block,
            resume_block,
            "Resuming from checkpoint"
        );
        resume_block
    } else {
        // No checkpoint found, start from chain tip
        provider.get_block_number().await?
    };

    // Create unified RPC block producer
    let block_producer = blocksource::RpcBlockProducer::new(
        provider.clone(),
        Duration::from_millis(cli.poll_interval_ms),
        start_block,
    );
    let block_producer = BlockProducerWrapper::new(block_producer, "rpc");

    build_node_common(cli, block_producer, tui_observer, tui_handle, provider, rpc_url).await
}

/// Build the node with common configuration.
pub async fn build_node_common<P: Provider<Optimism> + Clone + 'static>(
    cli: MontanaCli,
    block_producer: BlockProducerWrapper,
    tui_observer: Option<Arc<TuiObserver>>,
    tui_handle: Option<TuiHandle>,
    _provider: P,
    rpc_url: String,
) -> Result<Node<BlockProducerWrapper>> {
    // Build node config
    let config = NodeConfig {
        role: cli.mode.try_into().expect("Executor mode is not supported"),
        checkpoint_path: cli.checkpoint_path.clone(),
        checkpoint_interval_secs: cli.checkpoint_interval,
        skip_sync: cli.skip_sync,
    };

    // Build sync config
    let sync_config = SyncConfig { sync_threshold: cli.sync_threshold, ..Default::default() };

    // Use batch mode from CLI directly (it's already BatchSubmissionMode)
    let batch_mode = cli.batch_mode;

    // Build batch context
    let batch_inbox = Address::repeat_byte(0x42);
    let batch_ctx = Arc::new(
        BatchContext::new(batch_mode, batch_inbox)
            .await
            .map_err(|e| eyre::eyre!("Failed to create batch context: {}", e))?,
    );

    // Create node builder
    let mut builder = NodeBuilder::new().with_config(config.clone());

    // Create unbounded channel for full OpBlocks to decouple block fetching from execution
    // This allows the block feeder to continuously fetch blocks without blocking on executor pace
    // Using OpBlock preserves all block information for serialization in batches
    let (block_tx, block_rx) = mpsc::unbounded_channel::<OpBlock>();

    // Add sync stage OR spawn block feeder based on skip_sync
    if cli.skip_sync {
        // When skipping sync, we need to feed blocks directly to the sequencer
        // Spawn a DEDICATED THREAD with its own runtime for the block feeder.
        // This ensures true parallelism - the block feeder runs completely independently
        // of the node's main loop, preventing any blocking or starvation issues.
        let start = cli.start.unwrap_or(0);
        let poll_ms = cli.poll_interval_ms;
        let feeder_tui_handle = tui_handle.clone();
        let feeder_rpc_url = rpc_url.clone();

        // Send BlockBuilder initialization event to TUI if handle exists
        if let Some(ref handle) = tui_handle {
            handle.send(TuiEvent::BlockBuilderInit {
                rpc_url: rpc_url.clone(),
                poll_interval_ms: cli.poll_interval_ms,
            });
        }

        std::thread::spawn(move || {
            // Create a dedicated runtime for the block feeder
            let feeder_rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create block feeder runtime");

            feeder_rt.block_on(async move {
                // Create a new provider for this thread
                let feeder_provider = match ProviderBuilder::new()
                    .disable_recommended_fillers()
                    .network::<Optimism>()
                    .connect(&feeder_rpc_url)
                    .await
                {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!(error = %e, "Block feeder failed to connect to RPC");
                        return;
                    }
                };

                if let Err(e) =
                    run_block_feeder(feeder_provider, start, poll_ms, block_tx, feeder_tui_handle)
                        .await
                {
                    tracing::error!(error = %e, "Block feeder error");
                }
            });
        });
    } else {
        // Normal sync mode - add sync stage
        let sync_stage = SyncStage::new(block_producer, sync_config);
        builder = builder.with_sync_stage(sync_stage);
    }

    // Load checkpoint for TUI initialization display
    // In harness mode, always use default (no checkpoint) for fresh starts
    let checkpoint_for_tui = if cli.with_harness {
        Checkpoint::default()
    } else {
        Checkpoint::load(&cli.checkpoint_path).ok().flatten().unwrap_or_default()
    };

    // Create shared store for block transaction counts (shared between batch and derivation callbacks)
    // This allows the derivation callback to know tx counts that were recorded at batch submission time
    let tx_count_store = Arc::new(BlockTxCountStore::new());

    // Add sequencer if role requires it
    if config.role.runs_sequencer() {
        // Use optimized config for harness mode (faster batch submissions for demo/testing)
        let batcher_config = if cli.with_harness {
            BatcherConfig::builder()
                .min_batch_size(10) // 10 bytes (very low threshold for small harness txs)
                .batch_interval(std::time::Duration::from_secs(2)) // 2 seconds (quick feedback)
                .max_blocks_per_batch(3) // 3 blocks per batch (balance between responsiveness and throughput)
                .build()
        } else {
            BatcherConfig::default()
        };
        let compressor = BrotliCompressor::default();

        // Wrap the batch context sink in an adapter to bridge the two BatchSink traits
        let sink_adapter = BatchSinkAdapter::new(batch_ctx.sink());

        // In harness mode, disable checkpoint persistence for fresh starts every time
        let checkpoint_path =
            if cli.with_harness { None } else { Some(cli.checkpoint_path.clone()) };

        let mut sequencer = Sequencer::new(
            sink_adapter,
            compressor,
            batcher_config.clone(),
            checkpoint_path,
            block_rx,
        )?;

        // Wire up callbacks for TUI visibility if available
        if let Some(ref handle) = tui_handle {
            tracing::debug!("Wiring up TUI callbacks for sequencer");
            let exec_callback = Arc::new(TuiExecutionCallback::new(handle.clone()));
            let batch_callback =
                Arc::new(TuiBatchCallback::new(handle.clone(), Arc::clone(&tx_count_store)));
            sequencer = sequencer
                .with_execution_callback(exec_callback)
                .with_batch_callback(batch_callback);

            // Send sequencer initialization event to TUI
            handle.send(TuiEvent::SequencerInit {
                checkpoint_batch: checkpoint_for_tui.last_batch_submitted,
                min_batch_size: batcher_config.min_batch_size,
                batch_interval_secs: batcher_config.batch_interval.as_secs(),
                max_blocks_per_batch: batcher_config.max_blocks_per_batch,
            });

            // Send execution initialization event
            handle.send(TuiEvent::ExecutionInit {
                start_block: cli.start.unwrap_or(0),
                checkpoint_block: checkpoint_for_tui.last_block_executed,
                harness_mode: cli.with_harness,
            });
        } else {
            tracing::debug!("No TUI handle available, skipping callback setup");
        }

        builder = builder.with_sequencer(sequencer);
    }

    // Add validator if role requires it
    if config.role.runs_validator() {
        // Create a batch source adapter that wraps the batch context source
        // We need to create an owned adapter, not a reference-based one
        let source = BatchSourceAdapter::new(Arc::clone(&batch_ctx));
        let compressor = BrotliCompressor::default();

        // In harness mode, disable checkpoint persistence for fresh starts every time
        let checkpoint_path =
            if cli.with_harness { None } else { Some(cli.checkpoint_path.clone()) };

        // Create database for validator execution
        let data_dir = PathBuf::from("./montana-data/validator");
        std::fs::create_dir_all(&data_dir)?;

        let kvdb_path = data_dir.join("kvdb");
        let trie_path = data_dir.join("triedb");

        // Get genesis state (using DEFAULT_GENESIS_STATE for now)
        let anvil_state = AnvilState::from_json(DEFAULT_GENESIS_STATE)
            .map_err(|e| eyre::eyre!("Failed to parse anvil state: {}", e))?;
        let genesis = anvil_state.into_genesis();

        let kvdb = RocksDbKvDatabase::open_or_create(&kvdb_path, &genesis)
            .map_err(|e| eyre::eyre!("Failed to create kvdb: {}", e))?;
        let trie_db = TrieDatabase::open_or_create(&trie_path, &genesis, kvdb)
            .map_err(|e| eyre::eyre!("Failed to create trie db: {:?}", e))?;
        let cached_db = CachedDatabase::new(trie_db);
        let executor = BlockExecutor::new(cached_db, BASE_MAINNET);

        let mut validator =
            Validator::new(source, compressor, checkpoint_path, Box::new(executor))?;

        // Wire up derivation callback for TUI visibility if available
        if let Some(ref handle) = tui_handle {
            tracing::debug!("Wiring up TUI derivation callback for validator");
            let derivation_callback =
                Arc::new(TuiDerivationCallback::new(handle.clone(), Arc::clone(&tx_count_store)));
            validator = validator.with_derivation_callback(derivation_callback);

            // Send validator initialization event to TUI
            handle.send(TuiEvent::ValidatorInit {
                checkpoint_batch: checkpoint_for_tui.last_batch_derived,
            });
        }

        builder = builder.with_validator(validator);
    }

    // Add TUI observer if present
    if let Some(observer) = tui_observer {
        builder = builder.with_observer(observer);
    }

    builder.build()
}
