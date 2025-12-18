//! Ctrl+C signal handler utilities.
//!
//! This module provides utilities for installing a Ctrl+C (SIGINT) handler
//! that gracefully exits the application.

use std::sync::atomic::{AtomicBool, Ordering};

/// Global shutdown flag that indicates Ctrl+C has been received.
///
/// This is set by [`install_ctrlc_handler`] and can be checked by any component
/// that needs to respond to shutdown requests.
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Returns `true` if shutdown has been requested (via Ctrl+C).
///
/// Components can poll this to check if they should initiate graceful shutdown.
#[must_use]
pub fn is_shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::SeqCst)
}

/// Request shutdown programmatically (e.g., when TUI exits).
///
/// This sets the same flag as Ctrl+C, allowing components to respond uniformly.
pub fn request_shutdown() {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

/// Installs a Ctrl+C handler that sets the shutdown flag.
///
/// Unlike calling `process::exit()`, this allows the application to perform
/// graceful shutdown, including running destructors for child processes like anvil.
///
/// This function spawns a background task that waits for Ctrl+C and sets the
/// shutdown flag. It should be called early in the application startup.
///
/// # Examples
///
/// ```no_run
/// use montana_cli::{install_ctrlc_handler, is_shutdown_requested};
///
/// #[tokio::main]
/// async fn main() {
///     install_ctrlc_handler();
///
///     // Main loop checks for shutdown
///     loop {
///         if is_shutdown_requested() {
///             break;
///         }
///         // ... do work ...
///     }
///     // Destructors run here, cleaning up child processes
/// }
/// ```
pub fn install_ctrlc_handler() {
    tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "Failed to listen for Ctrl+C signal");
            return;
        }
        tracing::info!("Received Ctrl+C, initiating graceful shutdown...");
        SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
    });
}

/// Waits for Ctrl+C signal.
///
/// This is a lower-level function that allows callers to handle the signal
/// themselves rather than automatically exiting.
///
/// # Errors
///
/// Returns an error if the signal handler could not be installed.
///
/// # Examples
///
/// ```no_run
/// use montana_cli::wait_for_ctrlc;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     tokio::select! {
///         _ = wait_for_ctrlc() => {
///             println!("Shutting down...");
///         }
///         _ = async { /* main work */ } => {}
///     }
///     Ok(())
/// }
/// ```
pub async fn wait_for_ctrlc() -> std::io::Result<()> {
    tokio::signal::ctrl_c().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_ctrlc_handler_compiles() {
        // Just verify the function exists and compiles.
        // We can't actually test the signal handling behavior easily.
        let _ = install_ctrlc_handler as fn();
    }

    #[test]
    fn test_shutdown_request_functions() {
        // Test that shutdown request functions work
        // Note: This test may interfere with other tests if run in parallel,
        // but that's acceptable for a simple compile/function test.
        assert!(!is_shutdown_requested());
        request_shutdown();
        assert!(is_shutdown_requested());
        // Reset for other tests (best effort, not guaranteed in parallel tests)
        SHUTDOWN_REQUESTED.store(false, Ordering::SeqCst);
    }

    #[tokio::test]
    async fn test_wait_for_ctrlc_signature() {
        // Just verify the function exists and returns the expected type.
        // We can't actually trigger Ctrl+C in tests.
        use std::future::Future;
        fn assert_future<F: Future<Output = std::io::Result<()>>>(_: F) {}
        assert_future(wait_for_ctrlc());
    }
}
