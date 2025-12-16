//! Background task runners for batch submission and derivation.
//!
//! This module contains the async tasks that run batch submission
//! and derivation in the background, streaming blocks from an RPC
//! endpoint and updating the shared app state.

use std::{sync::Arc, time::Duration};

use montana_brotli::BrotliCompressor;
use montana_pipeline::{CompressedBatch, Compressor, L2BlockData};
use montana_zlib::ZlibCompressor;
use montana_zstd::ZstdCompressor;
use tokio::{sync::Mutex, time::sleep};

use crate::{
    Args,
    app::{App, LogEntry},
    rpc::RpcClient,
};

/// Get a compressor by name.
fn get_compressor(name: &str) -> Box<dyn Compressor + Send + Sync> {
    match name.to_lowercase().as_str() {
        "zlib" => Box::new(ZlibCompressor::balanced()),
        "zstd" => Box::new(ZstdCompressor::balanced()),
        _ => Box::new(BrotliCompressor::balanced()),
    }
}

/// Encode blocks into raw batch data.
fn encode_blocks(blocks: &[L2BlockData]) -> Vec<u8> {
    let mut raw_batch = Vec::new();
    for block in blocks {
        for tx in &block.transactions {
            raw_batch.extend_from_slice(&tx.0);
        }
    }
    raw_batch
}

/// Run the batch submission simulation, streaming blocks from RPC.
pub(crate) async fn run_batch_submission(app: Arc<Mutex<App>>, args: Args) {
    // Small delay to let UI initialize
    sleep(Duration::from_millis(100)).await;

    let rpc = RpcClient::new(args.rpc.clone());
    let compressor = get_compressor(&args.compression);
    let poll_interval = Duration::from_millis(args.poll_interval);

    // Get starting block
    let start_block = match args.start {
        Some(start) => start,
        None => {
            // Start from latest block
            match rpc.get_block_number().await {
                Ok(head) => {
                    let mut app_guard = app.lock().await;
                    app_guard
                        .log_batch(LogEntry::info(format!("Connected! Chain head at #{}", head)));
                    app_guard.set_chain_head(head);
                    head
                }
                Err(e) => {
                    let mut app_guard = app.lock().await;
                    app_guard.log_batch(LogEntry::error(format!("Failed to connect: {}", e)));
                    return;
                }
            }
        }
    };

    let mut current_block = start_block;
    let mut batch_number = 0u64;
    let mut pending_blocks: Vec<L2BlockData> = Vec::new();
    let mut pending_size = 0usize;

    {
        let mut app_guard = app.lock().await;
        app_guard.set_current_block(current_block);
        app_guard.log_batch(LogEntry::info(format!("Starting from block #{}", current_block)));
    }

    loop {
        // Check if paused
        {
            let app_guard = app.lock().await;
            if app_guard.is_paused {
                drop(app_guard);
                sleep(Duration::from_millis(100)).await;
                continue;
            }
        }

        // Update chain head periodically
        if let Ok(head) = rpc.get_block_number().await {
            let mut app_guard = app.lock().await;
            app_guard.set_chain_head(head);
        }

        // Try to fetch the next block
        match rpc.get_l2_block(current_block).await {
            Ok(block) => {
                let block_size: usize = block.transactions.iter().map(|tx| tx.0.len()).sum();
                let tx_count = block.transactions.len();

                {
                    let mut app_guard = app.lock().await;
                    app_guard.set_current_block(current_block);
                    app_guard.log_batch(LogEntry::info(format!(
                        "Block #{}: {} txs, {} bytes",
                        current_block, tx_count, block_size
                    )));
                }

                pending_blocks.push(block);
                pending_size += block_size;

                // Check if we should submit a batch
                let should_submit = pending_blocks.len() >= args.max_blocks_per_batch
                    || pending_size >= args.target_batch_size;

                if should_submit && !pending_blocks.is_empty() {
                    // Encode and compress the batch
                    let raw_data = encode_blocks(&pending_blocks);
                    let original_size = raw_data.len();
                    let blocks_in_batch = pending_blocks.len();

                    match compressor.compress(&raw_data) {
                        Ok(compressed) => {
                            let compressed_size = compressed.len();
                            let ratio = if original_size > 0 {
                                compressed_size as f64 / original_size as f64
                            } else {
                                1.0
                            };

                            // Update app state
                            {
                                let mut app_guard = app.lock().await;
                                app_guard.stats.batches_submitted += 1;
                                app_guard.stats.blocks_processed += blocks_in_batch as u64;
                                app_guard.stats.bytes_original += original_size;
                                app_guard.stats.bytes_compressed += compressed_size;
                                app_guard.stats.compression_ratio =
                                    if app_guard.stats.bytes_original > 0 {
                                        app_guard.stats.bytes_compressed as f64
                                            / app_guard.stats.bytes_original as f64
                                    } else {
                                        1.0
                                    };

                                app_guard.log_batch(LogEntry::info(format!(
                                    "Batch #{}: {} blocks, {} -> {} bytes ({:.1}%)",
                                    batch_number,
                                    blocks_in_batch,
                                    original_size,
                                    compressed_size,
                                    ratio * 100.0
                                )));

                                // Queue batch for derivation
                                let batch = CompressedBatch { batch_number, data: compressed };
                                app_guard.queue_batch(batch);
                            }

                            batch_number += 1;
                        }
                        Err(e) => {
                            let mut app_guard = app.lock().await;
                            app_guard
                                .log_batch(LogEntry::error(format!("Compression failed: {}", e)));
                        }
                    }

                    // Clear pending
                    pending_blocks.clear();
                    pending_size = 0;
                }

                current_block += 1;
            }
            Err(e) => {
                // Block not available yet (likely ahead of chain head)
                let is_not_found = matches!(e, crate::rpc::RpcError::BlockNotFound(_));
                if !is_not_found {
                    let mut app_guard = app.lock().await;
                    app_guard.log_batch(LogEntry::warn(format!("RPC error: {}", e)));
                }
                // Wait before retrying
                sleep(poll_interval).await;
            }
        }
    }
}

/// Run the derivation simulation.
pub(crate) async fn run_derivation(app: Arc<Mutex<App>>) {
    // Small delay to let batch submission start first
    sleep(Duration::from_millis(200)).await;

    let compression = {
        let app_guard = app.lock().await;
        app_guard.compression.clone()
    };

    let compressor = get_compressor(&compression);

    loop {
        // Check if paused
        {
            let app_guard = app.lock().await;
            if app_guard.is_paused {
                drop(app_guard);
                sleep(Duration::from_millis(100)).await;
                continue;
            }
        }

        // Try to get a batch
        let batch = {
            let mut app_guard = app.lock().await;
            app_guard.take_batch()
        };

        let Some(batch) = batch else {
            // No batch available, wait and try again
            sleep(Duration::from_millis(50)).await;
            continue;
        };

        // Decompress
        let compressed_size = batch.data.len();
        match compressor.decompress(&batch.data) {
            Ok(decompressed) => {
                let decompressed_size = decompressed.len();

                let mut app_guard = app.lock().await;
                app_guard.stats.batches_derived += 1;
                // We don't track individual blocks in derivation for simplicity
                app_guard.stats.blocks_derived += 1;
                app_guard.stats.bytes_decompressed += decompressed_size;
                app_guard.stats.derivation_healthy = true;

                app_guard.log_derivation(LogEntry::info(format!(
                    "Derived batch #{}: {} -> {} bytes",
                    batch.batch_number, compressed_size, decompressed_size
                )));
            }
            Err(e) => {
                let mut app_guard = app.lock().await;
                app_guard.stats.derivation_healthy = false;
                app_guard.log_derivation(LogEntry::error(format!(
                    "Failed to derive batch #{}: {}",
                    batch.batch_number, e
                )));
            }
        }
    }
}
