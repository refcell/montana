# migration

Database migration utilities for Montana.

## Overview

This crate provides utilities for migrating data from legacy MDBX databases (e.g., Reth's database format) to Montana's internal storage format.

## Core Components

| Component | Description |
|-----------|-------------|
| `Migrator` | Main migration orchestrator |
| `MigratorConfig` | Configuration for migration behavior |
| `MigrationStats` | Statistics from the migration process |
| `MigrationError` | Error types for migration failures |

## Usage

```rust,ignore
use migration::{Migrator, MigratorConfig};

// Build configuration
let config = MigratorConfig::default()
    .with_batch_size(10_000)
    .with_log_interval(100_000);

// Create migrator
let migrator = Migrator::with_config(&source_path, &dest_path, config)?;

// Run migration
let stats = migrator.migrate_all()?;

println!("Migrated {} accounts", stats.accounts_migrated);

// Close and finalize
migrator.close()?;
```

## Configuration

Default configuration values:

| Setting | Default | Description |
|---------|---------|-------------|
| `batch_size` | 10,000 | Entries per batch commit |
| `log_interval` | 100,000 | Entries between progress logs |

## License

Licensed under the MIT license.
