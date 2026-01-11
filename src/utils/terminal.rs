//! Terminal utilities

use anyhow::Result;
use crossterm::terminal;

/// Get current terminal size
pub fn get_terminal_size() -> Result<(u16, u16)> {
    let (cols, rows) = terminal::size()?;
    Ok((cols, rows))
}

/// Convert VT100 color to ratatui color
pub fn vt100_to_ratatui_color(color: vt100::Color) -> ratatui::style::Color {
    match color {
        vt100::Color::Default => ratatui::style::Color::Reset,
        vt100::Color::Idx(idx) => ratatui::style::Color::Indexed(idx),
        vt100::Color::Rgb(r, g, b) => ratatui::style::Color::Rgb(r, g, b),
    }
}

/// Truncate string to fit width with ellipsis
pub fn truncate(s: &str, max_width: usize) -> String {
    if s.len() <= max_width {
        s.to_string()
    } else if max_width <= 3 {
        ".".repeat(max_width)
    } else {
        format!("{}...", &s[..max_width - 3])
    }
}

/// Pad string to width
pub fn pad_right(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - s.len()))
    }
}

/// Center string in width
pub fn center(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        let padding = (width - s.len()) / 2;
        let extra = (width - s.len()) % 2;
        format!("{}{}{}", " ".repeat(padding), s, " ".repeat(padding + extra))
    }
}

/// Format bytes to human readable string
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format duration in seconds to human readable
pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}:{:02}", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}
