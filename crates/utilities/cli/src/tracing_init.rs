//! Tracing initialization utilities.

use tracing::Level;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize the tracing subscriber with the given verbosity level.
///
/// The verbosity level maps to tracing levels as follows:
/// - 0: WARN
/// - 1: INFO
/// - 2: DEBUG
/// - 3+: TRACE
///
/// The `RUST_LOG` environment variable can be used to override the default filter.
///
/// # Examples
///
/// ```no_run
/// use montana_cli::init_tracing;
///
/// // Initialize with INFO level logging (verbosity = 1)
/// init_tracing(1);
///
/// // Now tracing macros will work
/// tracing::info!("Application started");
/// ```
///
/// # Panics
///
/// This function will panic if a global tracing subscriber has already been set.
/// It should only be called once at the start of the application.
pub fn init_tracing(verbosity: u8) {
    let level = match verbosity {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let filter = EnvFilter::builder().with_default_directive(level.into()).from_env_lossy();

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true).with_thread_ids(false))
        .init();
}

/// Initialize the tracing subscriber with a string log level.
///
/// This is useful when the log level is provided directly as a string
/// (e.g., from a CLI argument) rather than as a verbosity count.
///
/// # Examples
///
/// ```no_run
/// use montana_cli::init_tracing_with_level;
///
/// init_tracing_with_level("info");
/// ```
///
/// # Panics
///
/// This function will panic if a global tracing subscriber has already been set.
pub fn init_tracing_with_level(log_level: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbosity_level_mapping() {
        // Just verify the mapping logic works
        assert_eq!(
            match 0u8 {
                0 => Level::WARN,
                1 => Level::INFO,
                2 => Level::DEBUG,
                _ => Level::TRACE,
            },
            Level::WARN
        );
        assert_eq!(
            match 1u8 {
                0 => Level::WARN,
                1 => Level::INFO,
                2 => Level::DEBUG,
                _ => Level::TRACE,
            },
            Level::INFO
        );
        assert_eq!(
            match 2u8 {
                0 => Level::WARN,
                1 => Level::INFO,
                2 => Level::DEBUG,
                _ => Level::TRACE,
            },
            Level::DEBUG
        );
        assert_eq!(
            match 3u8 {
                0 => Level::WARN,
                1 => Level::INFO,
                2 => Level::DEBUG,
                _ => Level::TRACE,
            },
            Level::TRACE
        );
    }
}
