//! Database migration utilities.
//!
//! This crate provides utilities for migrating data from legacy MDBX databases.

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod migrator;
pub use migrator::{
    DEFAULT_BATCH_SIZE, DEFAULT_LOG_INTERVAL, MigrationError, MigrationStats, Migrator,
    MigratorConfig,
};

pub mod reth;

pub use libmdbx;
