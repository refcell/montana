#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use clap::Parser;
use montana_brotli::BrotliCompressor;
use montana_cli::{Cli, CompressionAlgorithm, Mode, init_tracing};
use montana_local::{JsonSinkData, LocalBatchSink, LocalBatchSource};
use montana_pipeline::{BatchSink, BatchSource, CompressedBatch, Compressor, L2BlockData};
use montana_zlib::ZlibCompressor;
use montana_zstd::ZstdCompressor;

fn main() {
    let cli = Cli::parse();

    // Initialize tracing
    init_tracing(cli.verbose);

    if let Err(e) = run(cli) {
        tracing::error!("Pipeline failed: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Montana pipeline starting in {} mode", cli.mode);
    tracing::debug!("Input: {:?}", cli.input);
    tracing::debug!("Output: {:?}", cli.output);
    tracing::debug!("Compression: {}", cli.compression);

    // Create the tokio runtime
    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async {
        match cli.mode {
            Mode::Batch => run_batch_mode(&cli).await,
            Mode::Derivation => run_derivation_mode(&cli).await,
            Mode::Roundtrip => run_roundtrip_mode(&cli).await,
        }
    })
}

/// Run batch submission mode.
async fn run_batch_mode(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("=== Batch Submission Mode ===");

    // 1. Load the source data
    tracing::info!("Loading source data from {:?}", cli.input);
    let mut source = LocalBatchSource::from_file(&cli.input)?;

    // 2. Get pending blocks
    let blocks = source.pending_blocks().await?;
    tracing::info!("Loaded {} blocks", blocks.len());

    if blocks.is_empty() {
        tracing::warn!("No blocks to process");
        return Ok(());
    }

    // 3. Encode blocks (simple concatenation of transaction data)
    let raw_batch = encode_blocks(&blocks);
    tracing::info!("Encoded {} bytes of raw batch data", raw_batch.len());

    // 4. Handle compression based on CLI flag
    match cli.compression {
        CompressionAlgorithm::All => {
            run_all_compressions(&raw_batch, cli).await?;
        }
        algo => {
            run_single_compression(&raw_batch, algo, cli).await?;
        }
    }

    Ok(())
}

/// Run derivation mode - decompress batches from output file.
async fn run_derivation_mode(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("=== Derivation Mode ===");

    // Read the batch data from the output file (which was produced by batch mode)
    let sink_data = load_sink_data(&cli.output)?;

    if sink_data.batches.is_empty() {
        tracing::warn!("No batches to derive");
        return Ok(());
    }

    tracing::info!("Loaded {} batches to derive", sink_data.batches.len());

    let compressor = get_compressor(cli.compression)?;

    for batch in &sink_data.batches {
        tracing::info!("Deriving batch {}", batch.batch_number);

        // Parse hex data
        let compressed = hex_to_bytes(&batch.data)?;
        tracing::debug!("Compressed size: {} bytes", compressed.len());

        // Decompress
        let decompressed = compressor.decompress(&compressed)?;
        tracing::info!(
            "Decompressed batch {}: {} -> {} bytes",
            batch.batch_number,
            compressed.len(),
            decompressed.len()
        );
    }

    tracing::info!("Derivation complete!");
    Ok(())
}

/// Run roundtrip mode - batch submission followed by derivation with validation.
async fn run_roundtrip_mode(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("=== Roundtrip Mode ===");

    // 1. Load the source data
    tracing::info!("Loading source data from {:?}", cli.input);
    let mut source = LocalBatchSource::from_file(&cli.input)?;

    // 2. Get pending blocks
    let blocks = source.pending_blocks().await?;
    tracing::info!("Loaded {} blocks", blocks.len());

    if blocks.is_empty() {
        tracing::warn!("No blocks to process");
        return Ok(());
    }

    // 3. Encode blocks
    let original_batch = encode_blocks(&blocks);
    tracing::info!("Original batch size: {} bytes", original_batch.len());

    // 4. Compress (batch submission)
    let compressor = get_compressor(cli.compression)?;
    let compressed = compressor.compress(&original_batch)?;
    let ratio = compression_ratio(original_batch.len(), compressed.len());
    tracing::info!(
        "[{}] Compressed: {} -> {} bytes ({:.1}% ratio)",
        cli.compression,
        original_batch.len(),
        compressed.len(),
        ratio * 100.0
    );

    // 5. Decompress (derivation)
    let decompressed = compressor.decompress(&compressed)?;
    tracing::info!("Decompressed: {} bytes", decompressed.len());

    // 6. Validate roundtrip
    if original_batch == decompressed {
        tracing::info!("Roundtrip validation PASSED!");
        tracing::info!("  Original size: {} bytes", original_batch.len());
        tracing::info!("  Compressed size: {} bytes", compressed.len());
        tracing::info!("  Decompressed size: {} bytes", decompressed.len());
        tracing::info!("  Compression ratio: {:.1}%", ratio * 100.0);
    } else {
        tracing::error!("Roundtrip validation FAILED!");
        tracing::error!("  Original size: {} bytes", original_batch.len());
        tracing::error!("  Decompressed size: {} bytes", decompressed.len());

        // Find first difference
        for (i, (a, b)) in original_batch.iter().zip(decompressed.iter()).enumerate() {
            if a != b {
                tracing::error!(
                    "  First difference at byte {}: original=0x{:02x}, decompressed=0x{:02x}",
                    i,
                    a,
                    b
                );
                break;
            }
        }
        if original_batch.len() != decompressed.len() {
            tracing::error!(
                "  Length mismatch: original={}, decompressed={}",
                original_batch.len(),
                decompressed.len()
            );
        }

        return Err("Roundtrip validation failed".into());
    }

    // 7. Optionally write to output file
    let mut sink = LocalBatchSink::new(&cli.output);
    let batch = CompressedBatch {
        batch_number: 0,
        data: compressed,
        block_count: 0,
        first_block: 0,
        last_block: 0,
    };
    let receipt = sink.submit(batch).await?;

    tracing::info!("Batch submitted successfully!");
    tracing::info!("  Batch number: {}", receipt.batch_number);
    tracing::info!("  L1 block: {}", receipt.l1_block);
    tracing::info!("  Tx hash: {:?}", hex(&receipt.tx_hash));
    tracing::info!("Output written to {:?}", cli.output);

    Ok(())
}

/// Encode blocks into raw batch data (concatenation of transaction data).
fn encode_blocks(blocks: &[L2BlockData]) -> Vec<u8> {
    let mut raw_batch = Vec::new();
    let mut total_txs = 0;
    for block in blocks {
        for tx in &block.transactions {
            raw_batch.extend_from_slice(&tx.0);
            total_txs += 1;
        }
    }
    tracing::debug!("Encoded {} transactions", total_txs);
    raw_batch
}

/// Get the compressor for the given algorithm.
fn get_compressor(
    algo: CompressionAlgorithm,
) -> Result<Box<dyn Compressor + Send + Sync>, Box<dyn std::error::Error>> {
    match algo {
        CompressionAlgorithm::Brotli => Ok(Box::new(BrotliCompressor::balanced())),
        CompressionAlgorithm::Zlib => Ok(Box::new(ZlibCompressor::balanced())),
        CompressionAlgorithm::Zstd => Ok(Box::new(ZstdCompressor::balanced())),
        CompressionAlgorithm::All => {
            Err("Cannot use 'all' compression for derivation/roundtrip mode".into())
        }
    }
}

/// Load sink data from a JSON file.
fn load_sink_data(path: &std::path::Path) -> Result<JsonSinkData, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read sink file {:?}: {}", path, e))?;
    let data: JsonSinkData = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse sink file {:?}: {}", path, e))?;
    Ok(data)
}

/// Parse hex string to bytes.
fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|e| format!("Invalid hex at position {}: {}", i, e).into())
        })
        .collect()
}

/// Run compression with a single algorithm and submit to sink.
async fn run_single_compression(
    raw_batch: &[u8],
    algo: CompressionAlgorithm,
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    let compressor: Box<dyn Compressor + Send + Sync> = match algo {
        CompressionAlgorithm::Brotli => Box::new(BrotliCompressor::balanced()),
        CompressionAlgorithm::Zlib => Box::new(ZlibCompressor::balanced()),
        CompressionAlgorithm::Zstd => Box::new(ZstdCompressor::balanced()),
        CompressionAlgorithm::All => unreachable!(),
    };

    let compressed = compressor.compress(raw_batch)?;
    let ratio = compression_ratio(raw_batch.len(), compressed.len());

    tracing::info!(
        "[{}] Compressed: {} -> {} bytes ({:.1}% ratio)",
        algo,
        raw_batch.len(),
        compressed.len(),
        ratio * 100.0
    );

    // Submit to sink
    let mut sink = LocalBatchSink::new(&cli.output);
    let batch = CompressedBatch {
        batch_number: 0,
        data: compressed,
        block_count: 0,
        first_block: 0,
        last_block: 0,
    };
    let receipt = sink.submit(batch).await?;

    tracing::info!("Batch submitted successfully!");
    tracing::info!("  Batch number: {}", receipt.batch_number);
    tracing::info!("  L1 block: {}", receipt.l1_block);
    tracing::info!("  Tx hash: {:?}", hex(&receipt.tx_hash));
    if let Some(ref blob) = receipt.blob_hash {
        tracing::info!("  Blob hash: {:?}", hex(blob));
    }
    tracing::info!("Output written to {:?}", cli.output);

    Ok(())
}

/// Run all compression algorithms and compare results.
async fn run_all_compressions(
    raw_batch: &[u8],
    cli: &Cli,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Running all compression algorithms for comparison...\n");

    let mut results: Vec<(CompressionAlgorithm, usize, f64)> = Vec::new();

    for algo in CompressionAlgorithm::all_algorithms() {
        let compressor: Box<dyn Compressor + Send + Sync> = match algo {
            CompressionAlgorithm::Brotli => Box::new(BrotliCompressor::balanced()),
            CompressionAlgorithm::Zlib => Box::new(ZlibCompressor::balanced()),
            CompressionAlgorithm::Zstd => Box::new(ZstdCompressor::balanced()),
            CompressionAlgorithm::All => unreachable!(),
        };

        let compressed = compressor.compress(raw_batch)?;
        let ratio = compression_ratio(raw_batch.len(), compressed.len());

        results.push((algo, compressed.len(), ratio));

        tracing::info!(
            "[{}] {} -> {} bytes ({:.1}% ratio)",
            algo,
            raw_batch.len(),
            compressed.len(),
            ratio * 100.0
        );
    }

    // Sort by compressed size (best first)
    results.sort_by(|a, b| a.1.cmp(&b.1));

    tracing::info!("");
    tracing::info!("=== Compression Comparison ===");
    tracing::info!("Raw size: {} bytes", raw_batch.len());
    tracing::info!("");

    for (i, (algo, size, ratio)) in results.iter().enumerate() {
        let rank = if i == 0 { " (BEST)" } else { "" };
        tracing::info!("  {}: {} bytes ({:.1}% ratio){}", algo, size, ratio * 100.0, rank);
    }

    // Use the best algorithm for the actual submission
    let (best_algo, _, _) = results.first().unwrap();
    tracing::info!("");
    tracing::info!("Using {} for batch submission...", best_algo);

    let compressor: Box<dyn Compressor + Send + Sync> = match best_algo {
        CompressionAlgorithm::Brotli => Box::new(BrotliCompressor::balanced()),
        CompressionAlgorithm::Zlib => Box::new(ZlibCompressor::balanced()),
        CompressionAlgorithm::Zstd => Box::new(ZstdCompressor::balanced()),
        CompressionAlgorithm::All => unreachable!(),
    };

    let compressed = compressor.compress(raw_batch)?;

    // Submit to sink
    let mut sink = LocalBatchSink::new(&cli.output);
    let batch = CompressedBatch {
        batch_number: 0,
        data: compressed,
        block_count: 0,
        first_block: 0,
        last_block: 0,
    };
    let receipt = sink.submit(batch).await?;

    tracing::info!("Batch submitted successfully!");
    tracing::info!("  Batch number: {}", receipt.batch_number);
    tracing::info!("  L1 block: {}", receipt.l1_block);
    tracing::info!("  Tx hash: {:?}", hex(&receipt.tx_hash));
    if let Some(ref blob) = receipt.blob_hash {
        tracing::info!("  Blob hash: {:?}", hex(blob));
    }
    tracing::info!("Output written to {:?}", cli.output);

    Ok(())
}

/// Calculate compression ratio.
fn compression_ratio(original: usize, compressed: usize) -> f64 {
    if original == 0 { 1.0 } else { compressed as f64 / original as f64 }
}

/// Helper to format bytes as hex string.
fn hex(bytes: &[u8]) -> String {
    format!("0x{}", bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>())
}
