use tokio::sync::mpsc;

use crate::TuiEvent;

/// Handle for sending events to the TUI.
///
/// This handle wraps an unbounded channel sender and can be cloned and passed
/// to different parts of the Montana binary to send events to the TUI.
///
/// # Example
///
/// ```rust,ignore
/// use montana_tui::{create_tui, TuiEvent};
///
/// let (tui, handle) = create_tui();
///
/// // Clone the handle to send from multiple locations
/// let handle_clone = handle.clone();
///
/// // Send events from different components
/// handle.send(TuiEvent::UnsafeHeadUpdated(100));
/// handle_clone.send(TuiEvent::SafeHeadUpdated(90));
/// ```
#[derive(Debug, Clone)]
pub struct TuiHandle {
    /// Unbounded sender for TUI events
    tx: mpsc::UnboundedSender<TuiEvent>,
}

impl TuiHandle {
    /// Create a new TUI handle from an unbounded sender.
    ///
    /// This is typically called internally by [`create_tui`](crate::create_tui).
    /// Users should use [`create_tui`](crate::create_tui) instead.
    pub(crate) const fn new(tx: mpsc::UnboundedSender<TuiEvent>) -> Self {
        Self { tx }
    }

    /// Send an event to the TUI.
    ///
    /// This method never blocks. If the TUI has been dropped or is no longer
    /// receiving events, the send will silently fail.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to send to the TUI
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use montana_tui::{TuiEvent, TuiHandle};
    ///
    /// fn log_block(handle: &TuiHandle, number: u64) {
    ///     handle.send(TuiEvent::BlockBuilt {
    ///         number,
    ///         tx_count: 10,
    ///         size_bytes: 2048,
    ///         gas_used: 50000,
    ///     });
    /// }
    /// ```
    pub fn send(&self, event: TuiEvent) {
        let _ = self.tx.send(event);
    }
}
