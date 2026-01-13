//! Terminal utilities

use anyhow::Result;
use crossterm::terminal;
use std::process::Command;

/// Get current terminal size
pub fn get_terminal_size() -> Result<(u16, u16)> {
    let (cols, rows) = terminal::size()?;
    Ok((cols, rows))
}

/// Detect an available text editor
/// 
/// Checks in order:
/// 1. $EDITOR environment variable
/// 2. $VISUAL environment variable
/// 3. Common editors: nano, vim, vi, emacs, code, notepad (probes each)
/// 
/// Returns None if no editor is found
pub fn detect_editor() -> Option<String> {
    // Check EDITOR env var
    if let Ok(editor) = std::env::var("EDITOR") {
        if !editor.is_empty() && editor_exists(&editor) {
            return Some(editor);
        }
    }
    
    // Check VISUAL env var
    if let Ok(visual) = std::env::var("VISUAL") {
        if !visual.is_empty() && editor_exists(&visual) {
            return Some(visual);
        }
    }
    
    // Probe common editors in order of preference
    let common_editors = ["nano", "vim", "vi", "emacs", "code", "notepad"];
    
    for editor in common_editors {
        if editor_exists(editor) {
            return Some(editor.to_string());
        }
    }
    
    None
}

/// Check if an editor/command exists on the system
fn editor_exists(editor: &str) -> bool {
    // Extract just the command name (handle cases like "vim -u NONE")
    let cmd = editor.split_whitespace().next().unwrap_or(editor);
    
    // Use 'which' on Unix-like systems
    Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
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
