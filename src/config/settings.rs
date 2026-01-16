//! Application settings

use crate::tui::TerminalHighlightConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// UI settings
    #[serde(default)]
    pub ui: UiSettings,
    /// SSH settings
    #[serde(default)]
    pub ssh: SshSettings,
    /// Logging settings
    #[serde(default)]
    pub logging: LogSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ui: UiSettings::default(),
            ssh: SshSettings::default(),
            logging: LogSettings::default(),
        }
    }
}

/// UI settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettings {
    /// Theme name
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Enable mouse support
    #[serde(default = "default_true")]
    pub mouse_enabled: bool,
    /// Show status bar
    #[serde(default = "default_true")]
    pub show_status_bar: bool,
    /// Scrollback buffer size
    #[serde(default = "default_scrollback")]
    pub scrollback_lines: usize,
    /// Unicode graph symbols (braille, block, ascii)
    #[serde(default = "default_graph_style")]
    pub graph_style: String,
    /// Terminal keyword highlighting configuration
    #[serde(default)]
    pub terminal_highlight: TerminalHighlightConfig,
}

fn default_theme() -> String {
    "tokyo-night".to_string()
}

fn default_true() -> bool {
    true
}

fn default_scrollback() -> usize {
    10000
}

fn default_graph_style() -> String {
    "braille".to_string()
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            mouse_enabled: true,
            show_status_bar: true,
            scrollback_lines: default_scrollback(),
            graph_style: default_graph_style(),
            terminal_highlight: TerminalHighlightConfig::default(),
        }
    }
}

/// SSH settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshSettings {
    /// Default known hosts file
    #[serde(default = "default_known_hosts")]
    pub known_hosts_path: PathBuf,
    /// Connection timeout in seconds
    #[serde(default = "default_timeout")]
    pub connection_timeout: u32,
    /// Keep-alive interval in seconds (0 = disabled)
    #[serde(default = "default_keepalive")]
    pub keepalive_interval: u32,
    /// Reconnect attempts on disconnect
    #[serde(default = "default_reconnect")]
    pub reconnect_attempts: u32,
    /// Preferred authentication order
    #[serde(default = "default_auth_order")]
    pub auth_order: Vec<String>,
}

fn default_known_hosts() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ssh")
        .join("known_hosts")
}

fn default_timeout() -> u32 {
    30
}

fn default_keepalive() -> u32 {
    30
}

fn default_reconnect() -> u32 {
    3
}

fn default_auth_order() -> Vec<String> {
    vec![
        "agent".to_string(),
        "publickey".to_string(),
        "password".to_string(),
    ]
}

impl Default for SshSettings {
    fn default() -> Self {
        Self {
            known_hosts_path: default_known_hosts(),
            connection_timeout: default_timeout(),
            keepalive_interval: default_keepalive(),
            reconnect_attempts: default_reconnect(),
            auth_order: default_auth_order(),
        }
    }
}

/// Logging settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSettings {
    /// Enable session logging
    #[serde(default)]
    pub enabled: bool,
    /// Log directory
    #[serde(default = "default_log_dir")]
    pub directory: PathBuf,
    /// Log format (raw, timestamped)
    #[serde(default = "default_log_format")]
    pub format: String,
}

fn default_log_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rustyssh")
        .join("logs")
}

fn default_log_format() -> String {
    "timestamped".to_string()
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            directory: default_log_dir(),
            format: default_log_format(),
        }
    }
}
