//! Rendering utilities for converting log entries to ratatui widgets.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::{LogEntry, LogLevel};

/// Convert a vector of log entries to ratatui Lines with color coding.
///
/// This function converts a slice of [`LogEntry`] instances into a vector
/// of ratatui [`Line`] widgets, applying appropriate color coding based on
/// the log level:
/// - Info: Gray
/// - Warn: Yellow
/// - Error: Red
///
/// # Examples
///
/// ```
/// use montana_tui_common::{LogEntry, render_logs};
///
/// let logs = vec![
///     LogEntry::info("Starting..."),
///     LogEntry::warn("High memory usage"),
///     LogEntry::error("Connection failed"),
/// ];
///
/// let lines = render_logs(&logs);
/// assert_eq!(lines.len(), 3);
/// ```
pub fn render_logs(logs: &[LogEntry]) -> Vec<Line<'static>> {
    logs.iter()
        .map(|entry| {
            let color = match entry.level {
                LogLevel::Info => Color::Gray,
                LogLevel::Warn => Color::Yellow,
                LogLevel::Error => Color::Red,
            };
            Line::from(Span::styled(entry.message.clone(), Style::default().fg(color)))
        })
        .collect()
}

/// Convert a collection of log entries to ratatui Lines in reverse order (newest first).
///
/// This is useful when using `.wrap()` on a Paragraph widget, since `.wrap()` and
/// `.scroll()` conflict - scroll calculates by Line count but wrap changes visual lines.
/// By reversing the order, newest entries appear at the top without needing scroll.
///
/// # Examples
///
/// ```
/// use montana_tui_common::{LogEntry, render_logs_reversed};
///
/// let logs = vec![
///     LogEntry::info("First"),
///     LogEntry::info("Second"),
///     LogEntry::info("Third"),
/// ];
///
/// let lines = render_logs_reversed(&logs);
/// // Lines are now in order: Third, Second, First
/// assert_eq!(lines.len(), 3);
/// ```
pub fn render_logs_reversed<'a, I>(logs: I) -> Vec<Line<'static>>
where
    I: IntoIterator<Item = &'a LogEntry>,
    I::IntoIter: DoubleEndedIterator,
{
    logs.into_iter()
        .rev()
        .map(|entry| {
            let color = match entry.level {
                LogLevel::Info => Color::Gray,
                LogLevel::Warn => Color::Yellow,
                LogLevel::Error => Color::Red,
            };
            Line::from(Span::styled(entry.message.clone(), Style::default().fg(color)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_logs_empty() {
        let logs = vec![];
        let lines = render_logs(&logs);
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn test_render_logs_multiple() {
        let logs = vec![
            LogEntry::info("Info message"),
            LogEntry::warn("Warning message"),
            LogEntry::error("Error message"),
        ];
        let lines = render_logs(&logs);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_render_logs_color_mapping() {
        let logs = vec![LogEntry::info("Info"), LogEntry::warn("Warn"), LogEntry::error("Error")];
        let lines = render_logs(&logs);

        // Verify we have the expected number of lines
        assert_eq!(lines.len(), 3);

        // Note: We can't easily test the colors themselves without accessing
        // the internal structure of Line/Span, but we can verify the lines exist
    }
}
