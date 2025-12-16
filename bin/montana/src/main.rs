//! Montana Batch Submitter Execution Extension
//!
//! An execution extension that submits L2 batches to L1 as part of the OP Stack batcher pipeline.
//!
//! **Status:** This is currently a stub implementation. The batch submitter functionality is not yet implemented.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use clap::Parser;
use montana_cli::init_tracing;

/// Montana batch submitter CLI arguments.
#[derive(Parser, Debug)]
#[command(name = "montana")]
#[command(about = "Montana batch submitter execution extension")]
#[command(version)]
struct Args {
    /// Increase logging verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn main() {
    let args = Args::parse();

    // Initialize tracing
    init_tracing(args.verbose);

    tracing::info!("Montana batch submitter starting...");
    tracing::warn!("Batch submitter not yet implemented - this is a stub");

    // TODO: Implement execution extension logic
    // - Connect to L2 node
    // - Subscribe to new blocks
    // - Compress and batch transactions
    // - Submit to L1
}
