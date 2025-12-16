//! Benchmark for the local pipeline implementation.

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use montana_brotli::BrotliCompressor;
use montana_local::{LocalBatchSink, LocalBatchSource, NoopCompressor};
use montana_pipeline::{BatchSink, BatchSource, Compressor};
use montana_zlib::ZlibCompressor;
use montana_zstd::ZstdCompressor;

/// Sample JSON data for benchmarking with varying block counts.
fn generate_source_json(block_count: usize, tx_per_block: usize) -> String {
    let transactions: Vec<String> = (0..tx_per_block)
        .map(|i| format!(r#"{{"data": "0x{:02x}{:02x}{:02x}{:02x}"}}"#, i, i + 1, i + 2, i + 3))
        .collect();
    let tx_json = transactions.join(",");

    let blocks: Vec<String> = (0..block_count)
        .map(|i| {
            format!(r#"{{"timestamp": {}, "transactions": [{}]}}"#, 1000 + (i as u64 * 12), tx_json)
        })
        .collect();
    let blocks_json = blocks.join(",");

    format!(
        r#"{{
            "l1_origin": {{
                "block_number": 12345,
                "hash_prefix": "0x0102030405060708091011121314151617181920"
            }},
            "parent_hash": "0x2122232425262728293031323334353637383940",
            "blocks": [{}]
        }}"#,
        blocks_json
    )
}

fn bench_source_pending_blocks(c: &mut Criterion) {
    let mut group = c.benchmark_group("source_pending_blocks");

    for block_count in [1, 10, 100, 1000] {
        let json = generate_source_json(block_count, 5);

        group.throughput(Throughput::Elements(block_count as u64));
        group.bench_function(format!("{}_blocks", block_count), |b| {
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async {
                let mut source = LocalBatchSource::from_json(&json).unwrap();
                let blocks = source.pending_blocks().await.unwrap();
                black_box(blocks)
            });
        });
    }

    group.finish();
}

fn bench_sink_submit(c: &mut Criterion) {
    let mut group = c.benchmark_group("sink_submit");

    for data_size in [100, 1024, 10 * 1024, 100 * 1024] {
        let data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

        group.throughput(Throughput::Bytes(data_size as u64));
        group.bench_function(format!("{}_bytes", data_size), |b| {
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| {
                let data = data.clone();
                async move {
                    let mut sink = LocalBatchSink::in_memory();
                    let batch = montana_pipeline::CompressedBatch { batch_number: 1, data };
                    let receipt = sink.submit(batch).await.unwrap();
                    black_box(receipt)
                }
            });
        });
    }

    group.finish();
}

fn bench_compressor(c: &mut Criterion) {
    let mut group = c.benchmark_group("noop_compressor");

    for data_size in [100, 1024, 10 * 1024, 100 * 1024] {
        let data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();
        let compressor = NoopCompressor::new();

        group.throughput(Throughput::Bytes(data_size as u64));
        group.bench_function(format!("compress_{}_bytes", data_size), |b| {
            b.iter(|| {
                let result = compressor.compress(black_box(&data)).unwrap();
                black_box(result)
            });
        });

        group.bench_function(format!("decompress_{}_bytes", data_size), |b| {
            b.iter(|| {
                let result = compressor.decompress(black_box(&data)).unwrap();
                black_box(result)
            });
        });
    }

    group.finish();
}

fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_pipeline_noop");

    for block_count in [1, 10, 100] {
        let json = generate_source_json(block_count, 10);

        group.throughput(Throughput::Elements(block_count as u64));
        group.bench_function(format!("{}_blocks", block_count), |b| {
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| {
                let json = json.clone();
                async move {
                    // Source: read blocks
                    let mut source = LocalBatchSource::from_json(&json).unwrap();
                    let blocks = source.pending_blocks().await.unwrap();

                    // Encode: simulate batch encoding (just concatenate tx data)
                    let mut raw_batch = Vec::new();
                    for block in &blocks {
                        for tx in &block.transactions {
                            raw_batch.extend_from_slice(&tx.0);
                        }
                    }

                    // Compress: noop compressor
                    let compressor = NoopCompressor::new();
                    let compressed = compressor.compress(&raw_batch).unwrap();

                    // Sink: submit batch
                    let mut sink = LocalBatchSink::in_memory();
                    let batch =
                        montana_pipeline::CompressedBatch { batch_number: 0, data: compressed };
                    let receipt = sink.submit(batch).await.unwrap();

                    black_box(receipt)
                }
            });
        });
    }

    group.finish();
}

fn bench_brotli_compressor(c: &mut Criterion) {
    let mut group = c.benchmark_group("brotli_compressor");

    // Use smaller data sizes for brotli since it's slower
    for data_size in [100, 1024, 10 * 1024] {
        let data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

        // Benchmark fast preset
        let fast = BrotliCompressor::fast();
        group.throughput(Throughput::Bytes(data_size as u64));
        group.bench_function(format!("fast_compress_{}_bytes", data_size), |b| {
            b.iter(|| {
                let result = fast.compress(black_box(&data)).unwrap();
                black_box(result)
            });
        });

        // Benchmark balanced preset
        let balanced = BrotliCompressor::balanced();
        group.bench_function(format!("balanced_compress_{}_bytes", data_size), |b| {
            b.iter(|| {
                let result = balanced.compress(black_box(&data)).unwrap();
                black_box(result)
            });
        });

        // Benchmark decompression (use fast-compressed data)
        let compressed = fast.compress(&data).unwrap();
        group.bench_function(format!("decompress_{}_bytes", data_size), |b| {
            b.iter(|| {
                let result = fast.decompress(black_box(&compressed)).unwrap();
                black_box(result)
            });
        });
    }

    group.finish();
}

fn bench_full_pipeline_brotli(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_pipeline_brotli");

    for block_count in [1, 10, 100] {
        let json = generate_source_json(block_count, 10);

        group.throughput(Throughput::Elements(block_count as u64));
        group.bench_function(format!("{}_blocks", block_count), |b| {
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| {
                let json = json.clone();
                async move {
                    // Source: read blocks
                    let mut source = LocalBatchSource::from_json(&json).unwrap();
                    let blocks = source.pending_blocks().await.unwrap();

                    // Encode: simulate batch encoding (just concatenate tx data)
                    let mut raw_batch = Vec::new();
                    for block in &blocks {
                        for tx in &block.transactions {
                            raw_batch.extend_from_slice(&tx.0);
                        }
                    }

                    // Compress: brotli compressor (fast preset for benchmarks)
                    let compressor = BrotliCompressor::fast();
                    let compressed = compressor.compress(&raw_batch).unwrap();

                    // Sink: submit batch
                    let mut sink = LocalBatchSink::in_memory();
                    let batch =
                        montana_pipeline::CompressedBatch { batch_number: 0, data: compressed };
                    let receipt = sink.submit(batch).await.unwrap();

                    black_box(receipt)
                }
            });
        });
    }

    group.finish();
}

fn bench_zlib_compressor(c: &mut Criterion) {
    let mut group = c.benchmark_group("zlib_compressor");

    for data_size in [100, 1024, 10 * 1024] {
        let data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

        // Benchmark fast preset
        let fast = ZlibCompressor::fast();
        group.throughput(Throughput::Bytes(data_size as u64));
        group.bench_function(format!("fast_compress_{}_bytes", data_size), |b| {
            b.iter(|| {
                let result = fast.compress(black_box(&data)).unwrap();
                black_box(result)
            });
        });

        // Benchmark balanced preset
        let balanced = ZlibCompressor::balanced();
        group.bench_function(format!("balanced_compress_{}_bytes", data_size), |b| {
            b.iter(|| {
                let result = balanced.compress(black_box(&data)).unwrap();
                black_box(result)
            });
        });

        // Benchmark decompression (use fast-compressed data)
        let compressed = fast.compress(&data).unwrap();
        group.bench_function(format!("decompress_{}_bytes", data_size), |b| {
            b.iter(|| {
                let result = fast.decompress(black_box(&compressed)).unwrap();
                black_box(result)
            });
        });
    }

    group.finish();
}

fn bench_full_pipeline_zlib(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_pipeline_zlib");

    for block_count in [1, 10, 100] {
        let json = generate_source_json(block_count, 10);

        group.throughput(Throughput::Elements(block_count as u64));
        group.bench_function(format!("{}_blocks", block_count), |b| {
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| {
                let json = json.clone();
                async move {
                    // Source: read blocks
                    let mut source = LocalBatchSource::from_json(&json).unwrap();
                    let blocks = source.pending_blocks().await.unwrap();

                    // Encode: simulate batch encoding (just concatenate tx data)
                    let mut raw_batch = Vec::new();
                    for block in &blocks {
                        for tx in &block.transactions {
                            raw_batch.extend_from_slice(&tx.0);
                        }
                    }

                    // Compress: zlib compressor (fast preset for benchmarks)
                    let compressor = ZlibCompressor::fast();
                    let compressed = compressor.compress(&raw_batch).unwrap();

                    // Sink: submit batch
                    let mut sink = LocalBatchSink::in_memory();
                    let batch =
                        montana_pipeline::CompressedBatch { batch_number: 0, data: compressed };
                    let receipt = sink.submit(batch).await.unwrap();

                    black_box(receipt)
                }
            });
        });
    }

    group.finish();
}

fn bench_zstd_compressor(c: &mut Criterion) {
    let mut group = c.benchmark_group("zstd_compressor");

    for data_size in [100, 1024, 10 * 1024] {
        let data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

        // Benchmark fast preset
        let fast = ZstdCompressor::fast();
        group.throughput(Throughput::Bytes(data_size as u64));
        group.bench_function(format!("fast_compress_{}_bytes", data_size), |b| {
            b.iter(|| {
                let result = fast.compress(black_box(&data)).unwrap();
                black_box(result)
            });
        });

        // Benchmark balanced preset
        let balanced = ZstdCompressor::balanced();
        group.bench_function(format!("balanced_compress_{}_bytes", data_size), |b| {
            b.iter(|| {
                let result = balanced.compress(black_box(&data)).unwrap();
                black_box(result)
            });
        });

        // Benchmark decompression (use fast-compressed data)
        let compressed = fast.compress(&data).unwrap();
        group.bench_function(format!("decompress_{}_bytes", data_size), |b| {
            b.iter(|| {
                let result = fast.decompress(black_box(&compressed)).unwrap();
                black_box(result)
            });
        });
    }

    group.finish();
}

fn bench_full_pipeline_zstd(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_pipeline_zstd");

    for block_count in [1, 10, 100] {
        let json = generate_source_json(block_count, 10);

        group.throughput(Throughput::Elements(block_count as u64));
        group.bench_function(format!("{}_blocks", block_count), |b| {
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| {
                let json = json.clone();
                async move {
                    // Source: read blocks
                    let mut source = LocalBatchSource::from_json(&json).unwrap();
                    let blocks = source.pending_blocks().await.unwrap();

                    // Encode: simulate batch encoding (just concatenate tx data)
                    let mut raw_batch = Vec::new();
                    for block in &blocks {
                        for tx in &block.transactions {
                            raw_batch.extend_from_slice(&tx.0);
                        }
                    }

                    // Compress: zstd compressor (fast preset for benchmarks)
                    let compressor = ZstdCompressor::fast();
                    let compressed = compressor.compress(&raw_batch).unwrap();

                    // Sink: submit batch
                    let mut sink = LocalBatchSink::in_memory();
                    let batch =
                        montana_pipeline::CompressedBatch { batch_number: 0, data: compressed };
                    let receipt = sink.submit(batch).await.unwrap();

                    black_box(receipt)
                }
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_source_pending_blocks,
    bench_sink_submit,
    bench_compressor,
    bench_full_pipeline,
    bench_brotli_compressor,
    bench_full_pipeline_brotli,
    bench_zlib_compressor,
    bench_full_pipeline_zlib,
    bench_zstd_compressor,
    bench_full_pipeline_zstd,
);

criterion_main!(benches);
