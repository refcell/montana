#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::path::PathBuf;

use clap::Parser;
use eyre::Result;
use migration::{Migrator, MigratorConfig};
use tracing::info;

/// Migrate Reth MDBX database to TrieDB.
#[derive(Parser, Debug)]
#[command(name = "migrate")]
#[command(about = "Migrate Reth MDBX database to TrieDB")]
struct Args {
    /// Path to the source Reth MDBX database.
    #[arg(short, long)]
    source: PathBuf,

    /// Path to the destination TrieDB database.
    #[arg(short, long)]
    dest: PathBuf,

    /// Batch size for commits (number of entries per transaction).
    #[arg(long, default_value_t = migration::DEFAULT_BATCH_SIZE)]
    batch_size: u64,

    /// Interval for progress logging (number of entries between logs).
    #[arg(long, default_value_t = migration::DEFAULT_LOG_INTERVAL)]
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
        "starting migration"
    );

    // Build configuration
    let config = MigratorConfig::default()
        .with_batch_size(args.batch_size)
        .with_log_interval(args.log_interval);

    // Create migrator
    let migrator = Migrator::with_config(&args.source, &args.dest, config)?;

    // Run migration
    let stats = migrator.migrate_all()?;

    // Get state root before closing
    let state_root = migrator.state_root();

    info!(
        accounts = stats.accounts_migrated,
        account_errors = stats.account_errors,
        storage_slots = stats.storage_slots_migrated,
        storage_errors = stats.storage_errors,
        %state_root,
        "migration complete"
    );

    if stats.account_errors > 0 || stats.storage_errors > 0 {
        tracing::warn!("some entries failed to migrate, check logs for details");
    }

    // Close migrator
    migrator.close()?;

    Ok(())
}
