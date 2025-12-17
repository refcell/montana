//! Migrate Reth MDBX database to TrieDB.
//!
//! This tool reads account and storage state from a Reth MDBX database
//! and writes it to a new TrieDB database.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p migrate -- --source /path/to/reth/db --dest /path/to/triedb
//! ```

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

    /// Path to checkpoint file for resume capability.
    /// If not specified, defaults to <dest>.checkpoint
    #[arg(short, long)]
    checkpoint: Option<PathBuf>,

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

    // Determine checkpoint path
    let checkpoint_path = args.checkpoint.unwrap_or_else(|| {
        let mut path = args.dest.clone();
        path.set_extension("checkpoint");
        path
    });

    info!(
        source = %args.source.display(),
        dest = %args.dest.display(),
        checkpoint = %checkpoint_path.display(),
        batch_size = args.batch_size,
        log_interval = args.log_interval,
        "starting migration"
    );

    // Build configuration
    let config = MigratorConfig::default()
        .with_batch_size(args.batch_size)
        .with_log_interval(args.log_interval)
        .with_checkpoint_path(&checkpoint_path);

    // Create migrator
    let migrator = Migrator::with_config(&args.source, &args.dest, config)?;

    // Run migration
    let stats = migrator.migrate_all()?;

    info!(
        accounts = stats.accounts_migrated,
        account_errors = stats.account_errors,
        storage_slots = stats.storage_slots_migrated,
        storage_errors = stats.storage_errors,
        "migration complete"
    );

    // Print summary
    println!("\n=== Migration Summary ===");
    println!("Accounts migrated:      {}", stats.accounts_migrated);
    println!("Account errors:         {}", stats.account_errors);
    println!("Storage slots migrated: {}", stats.storage_slots_migrated);
    println!("Storage errors:         {}", stats.storage_errors);

    if stats.account_errors > 0 || stats.storage_errors > 0 {
        println!("\nWarning: Some entries failed to migrate. Check logs for details.");
    }

    // Close migrator
    migrator.close()?;

    Ok(())
}
