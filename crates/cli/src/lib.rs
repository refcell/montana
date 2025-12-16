#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod cli;
pub use cli::Cli;

mod compression;
pub use compression::CompressionAlgorithm;

mod tracing;
pub use tracing::init_tracing;
