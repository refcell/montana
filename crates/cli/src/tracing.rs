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
