//! Ctrl+C signal handler utilities.
//!
//! This module provides utilities for installing a Ctrl+C (SIGINT) handler
//! that gracefully exits the application.

use std::process;

/// Installs a Ctrl+C handler that exits the process with code 130.
///
/// Exit code 130 is the conventional exit code for processes terminated by SIGINT
/// (128 + signal number 2).
///
/// This function spawns a background task that waits for Ctrl+C and then exits.
/// It should be called early in the application startup.
///
/// # Examples
///
/// ```no_run
/// use montana_cli::install_ctrlc_handler;
///
/// #[tokio::main]
/// async fn main() {
///     install_ctrlc_handler();
///     // ... rest of application
/// }
/// ```
pub fn install_ctrlc_handler() {
    tokio::spawn(async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "Failed to listen for Ctrl+C signal");
            return;
        }
        tracing::info!("Received Ctrl+C, exiting...");
        process::exit(130);
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

    #[tokio::test]
    async fn test_wait_for_ctrlc_signature() {
        // Just verify the function exists and returns the expected type.
        // We can't actually trigger Ctrl+C in tests.
        use std::future::Future;
        fn assert_future<F: Future<Output = std::io::Result<()>>>(_: F) {}
        assert_future(wait_for_ctrlc());
    }
}
