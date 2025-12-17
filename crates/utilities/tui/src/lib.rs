#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// These will be used in the full TUI implementation
#[cfg(not(test))]
use {crossterm as _, ratatui as _};

/// TUI events.
///
/// The [`TuiEvent`] enum defines all events that can be sent from the Montana
/// binary to the TUI. These events represent key activities like transaction
/// pool updates, block building, batch submission, and block derivation.
mod events;
pub use events::TuiEvent;

/// TUI event handle.
///
/// The [`TuiHandle`] struct provides a handle for sending events to the TUI.
/// It wraps an unbounded channel sender and can be cloned to send events from
/// multiple locations in the Montana binary.
mod handle;
pub use handle::TuiHandle;

/// Application state.
///
/// The [`App`] struct maintains all state for the Montana TUI, including chain
/// head progression, statistics, latency metrics, and log buffers for each
/// component.
mod app;
pub use app::App;

/// TUI implementation.
///
/// The [`MontanaTui`] struct manages the TUI event loop and rendering. The
/// [`create_tui`] function creates both a TUI instance and a handle for sending
/// events.
mod tui;
pub use tui::{MontanaTui, create_tui};
