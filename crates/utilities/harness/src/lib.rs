#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod anvil_state;
pub use anvil_state::{AnvilAccountRecord, AnvilBlockEnv, AnvilState, AnvilStateError};

mod config;
pub use config::{DEFAULT_GENESIS_STATE, HarnessConfig};

mod harness;
pub use harness::Harness;

mod progress;
pub use progress::{BoxedProgressReporter, HarnessProgressReporter};
