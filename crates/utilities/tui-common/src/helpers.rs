//! Helper functions for formatting data in the TUI.

/// Format a byte count as a human-readable string.
///
/// Converts byte counts to B, KB, or MB based on size.
///
/// # Examples
///
/// ```
/// use montana_tui_common::format_bytes;
///
/// assert_eq!(format_bytes(512), "512 B");
/// assert_eq!(format_bytes(2048), "2.00 KB");
/// assert_eq!(format_bytes(2097152), "2.00 MB");
/// ```
pub fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Truncate a URL to a maximum length for display.
///
/// If the URL is longer than `max_len`, it will be truncated with "..." appended.
/// The total length including "..." will be `max_len + 3`.
///
/// # Examples
///
/// ```
/// use montana_tui_common::truncate_url;
///
/// let url = "https://example.com/very/long/path";
/// assert_eq!(truncate_url(url, 20), "https://example.com/...");
/// ```
pub fn truncate_url(url: &str, max_len: usize) -> String {
    if url.len() <= max_len { url.to_string() } else { format!("{}...", &url[..max_len]) }
}

/// Format a duration in milliseconds as a human-readable string.
///
/// # Examples
///
/// ```
/// use montana_tui_common::format_duration_ms;
///
/// assert_eq!(format_duration_ms(1234), "1234.0ms");
/// assert_eq!(format_duration_ms(42), "42.0ms");
/// ```
pub fn format_duration_ms(ms: u64) -> String {
    format!("{}.0ms", ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(2048), "2.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(2 * 1024 * 1024), "2.00 MB");
    }

    #[test]
    fn test_truncate_url() {
        let short_url = "https://example.com";
        assert_eq!(truncate_url(short_url, 50), short_url);

        let long_url = "https://example.com/very/long/path/to/resource";
        assert_eq!(truncate_url(long_url, 20), "https://example.com/...");
        assert_eq!(truncate_url(long_url, 100), long_url);
    }

    #[test]
    fn test_format_duration_ms() {
        assert_eq!(format_duration_ms(0), "0.0ms");
        assert_eq!(format_duration_ms(42), "42.0ms");
        assert_eq!(format_duration_ms(1234), "1234.0ms");
    }
}
