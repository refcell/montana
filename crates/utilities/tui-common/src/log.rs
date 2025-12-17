//! Log level and log entry types for TUI display.

/// Log level for display in the TUI.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LogLevel {
    /// Informational message.
    #[default]
    Info,
    /// Warning message.
    Warn,
    /// Error message.
    Error,
}

/// A log entry for display in the TUI.
///
/// Each log entry consists of a severity level and a message string.
/// Log entries can be created using the convenience constructors
/// [`LogEntry::info`], [`LogEntry::warn`], and [`LogEntry::error`].
#[derive(Clone, Debug)]
pub struct LogEntry {
    /// The log level.
    pub level: LogLevel,
    /// The log message.
    pub message: String,
}

impl LogEntry {
    /// Create a new info log entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use montana_tui_common::LogEntry;
    ///
    /// let entry = LogEntry::info("Connection established");
    /// ```
    pub fn info(message: impl Into<String>) -> Self {
        Self { level: LogLevel::Info, message: message.into() }
    }

    /// Create a new warning log entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use montana_tui_common::LogEntry;
    ///
    /// let entry = LogEntry::warn("High latency detected");
    /// ```
    pub fn warn(message: impl Into<String>) -> Self {
        Self { level: LogLevel::Warn, message: message.into() }
    }

    /// Create a new error log entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use montana_tui_common::LogEntry;
    ///
    /// let entry = LogEntry::error("Connection failed");
    /// ```
    pub fn error(message: impl Into<String>) -> Self {
        Self { level: LogLevel::Error, message: message.into() }
    }
}
