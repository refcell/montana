#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod headless;
pub use headless::run_headless;

mod tui;
pub use tui::run_with_tui;

mod builder;
pub use builder::{build_node, build_node_common};
