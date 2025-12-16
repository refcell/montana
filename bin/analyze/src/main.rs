#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use clap::Parser;
use montana_brotli::BrotliCompressor;
use montana_cli::{Cli, CompressionAlgorithm, init_tracing};
use montana_local::{LocalBatchSink, LocalBatchSource};
use montana_pipeline::{BatchSink, BatchSource, CompressedBatch, Compressor};
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
    tracing::info!("Montana batch submission pipeline starting");
    tracing::debug!("Input: {:?}", cli.input);
    tracing::debug!("Output: {:?}", cli.output);
    tracing::debug!("Compression: {}", cli.compression);

    // Create the tokio runtime
    let rt = tokio::runtime::Runtime::new()?;

    rt.block_on(async {
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

        // 3. Get metadata
        let l1_origin = source.l1_origin().await?;
        let l1_origin_hash = source.l1_origin_hash().await?;
        let parent_hash = source.parent_hash().await?;
        tracing::debug!("L1 origin: {} ({:?})", l1_origin, hex(&l1_origin_hash));
        tracing::debug!("Parent hash: {:?}", hex(&parent_hash));

        // 4. Encode blocks (simple concatenation of transaction data)
        let mut raw_batch = Vec::new();
        let mut total_txs = 0;
        for block in &blocks {
            for tx in &block.transactions {
                raw_batch.extend_from_slice(&tx.0);
                total_txs += 1;
            }
        }
        tracing::info!("Encoded {} transactions ({} bytes)", total_txs, raw_batch.len());

        // 5. Handle compression based on CLI flag
        match cli.compression {
            CompressionAlgorithm::All => {
                run_all_compressions(&raw_batch, &cli).await?;
            }
            algo => {
                run_single_compression(&raw_batch, algo, &cli).await?;
            }
        }

        Ok(())
    })
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
    let batch = CompressedBatch { batch_number: 0, data: compressed };
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
    let batch = CompressedBatch { batch_number: 0, data: compressed };
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
