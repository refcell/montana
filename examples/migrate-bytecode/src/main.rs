#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::{path::PathBuf, time::Instant};

use alloy::genesis::Genesis;
use alloy_primitives::B256;
use clap::Parser;
use database::{KeyValueDatabase, RocksDbKvDatabase};
use eyre::Result;
use migration::libmdbx::{Database, DatabaseOptions, Mode, NoWriteMap};
use tracing::info;

/// Default batch size for commits.
const DEFAULT_BATCH_SIZE: u64 = 10_000;

/// Default interval for progress logging.
const DEFAULT_LOG_INTERVAL: u64 = 100_000;

/// Maximum number of tables to open in the source MDBX database.
const MAX_MDBX_TABLES: u64 = 16;

/// Migrate Reth Bytecodes table to RocksDB.
#[derive(Parser, Debug)]
#[command(name = "migrate-bytecode")]
#[command(about = "Migrate Reth Bytecodes table to RocksDB")]
struct Args {
    /// Path to the source Reth MDBX database.
    #[arg(short, long)]
    source: PathBuf,

    /// Path to the destination RocksDB database.
    #[arg(short, long)]
    dest: PathBuf,

    /// Batch size for commits (number of entries per flush).
    #[arg(long, default_value_t = DEFAULT_BATCH_SIZE)]
    batch_size: u64,

    /// Interval for progress logging (number of entries between logs).
    #[arg(long, default_value_t = DEFAULT_LOG_INTERVAL)]
    log_interval: u64,
}

fn main() -> Result<()> {
    // Initialize tracing with info level by default
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let args = Args::parse();

    info!(
        source = %args.source.display(),
        dest = %args.dest.display(),
        batch_size = args.batch_size,
        log_interval = args.log_interval,
        "starting bytecode migration"
    );

    // Open source Reth MDBX in read-only mode
    let source_opts = DatabaseOptions {
        mode: Mode::ReadOnly,
        max_tables: Some(MAX_MDBX_TABLES),
        ..Default::default()
    };
    let source = Database::<NoWriteMap>::open_with_options(&args.source, source_opts)?;

    // Open destination RocksDB using the database crate
    let genesis = Genesis::default();
    let dest = RocksDbKvDatabase::open_or_create(&args.dest, &genesis)?;

    // Begin read transaction on source
    let src_txn = source.begin_ro_txn()?;
    let table = src_txn.open_table(Some("Bytecodes"))?;
    let mut cursor = src_txn.cursor(&table)?;

    let start_time = Instant::now();
    let mut migrated: u64 = 0;
    let mut errors: u64 = 0;

    // Iterate through all bytecode entries
    // Key: B256 (code hash), Value: raw bytecode bytes
    for result in cursor.iter_start::<[u8; 32], Vec<u8>>() {
        match result {
            Ok((key, value)) => {
                let code_hash = B256::from(key);

                // Store as code_hash -> raw bytecode
                dest.set_code(&code_hash, &value)?;
                migrated += 1;

                // Log progress
                if migrated % args.log_interval == 0 {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let rate = migrated as f64 / elapsed;
                    info!(
                        migrated,
                        errors,
                        rate = format!("{:.0}/s", rate),
                        "bytecode migration progress"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to read bytecode entry");
                errors += 1;
            }
        }
    }

    let elapsed = start_time.elapsed();
    info!(migrated, errors, elapsed_secs = elapsed.as_secs(), "bytecode migration complete");

    if errors > 0 {
        tracing::warn!("some entries failed to migrate, check logs for details");
    }

    Ok(())
}
