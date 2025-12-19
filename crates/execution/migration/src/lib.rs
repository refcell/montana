#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod migrator;
pub use migrator::{
    DEFAULT_BATCH_SIZE, DEFAULT_LOG_INTERVAL, MigrationError, MigrationStats, Migrator,
    MigratorConfig,
};

pub mod reth;

pub use libmdbx;
