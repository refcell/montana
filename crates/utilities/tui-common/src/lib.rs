#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

/// Log level and log entry types for TUI display.
///
/// The [`LogLevel`] enum and [`LogEntry`] struct provide a simple
/// logging system for displaying messages in the TUI with different
/// severity levels (info, warning, error).
mod log;
pub use log::{LogEntry, LogLevel};

/// Helper functions for formatting data in the TUI.
///
/// The [`format_bytes`], [`truncate_url`], and [`format_duration_ms`]
/// functions provide utilities for formatting data for display in the TUI.
mod helpers;
pub use helpers::{format_bytes, format_duration_ms, truncate_url};

/// Rendering utilities for converting log entries to ratatui widgets.
///
/// The [`render_logs`] function converts a vector of [`LogEntry`] instances
/// into ratatui `Line` widgets with appropriate color coding based on log level.
mod render;
pub use render::render_logs;
