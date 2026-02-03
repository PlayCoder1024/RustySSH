//! Application logging configuration
//!
//! Provides file-based logging using tracing-appender with daily rotation.
//! Logs are written to `~/.local/share/rustyssh/logs/` to keep the TUI clean.

use crate::config::LogSettings;
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize file-based logging.
///
/// Returns a guard that must be kept alive for the application duration.
/// If logging is disabled, returns None and no logs are written.
///
/// # Arguments
/// * `settings` - Logging settings from the application config
///
/// # Returns
/// * `Option<WorkerGuard>` - Guard that flushes logs on drop, or None if disabled
pub fn init(settings: &LogSettings) -> Option<WorkerGuard> {
    if !settings.enabled {
        return None;
    }

    // Ensure log directory exists
    let log_dir = get_log_directory(&settings.directory);
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Failed to create log directory {:?}: {}", log_dir, e);
        return None;
    }

    // Create daily rolling file appender
    let file_appender = tracing_appender::rolling::daily(&log_dir, "app.log");

    // Use non-blocking writer to avoid blocking the TUI
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Build the subscriber with file output only
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,rustyssh=debug"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(false)
                .with_file(true)
                .with_line_number(true),
        )
        .init();

    tracing::info!("Logging initialized, writing to {:?}", log_dir);

    Some(guard)
}

/// Get the log directory path, using the configured directory or default.
fn get_log_directory(configured: &PathBuf) -> PathBuf {
    if configured.as_os_str().is_empty() {
        // Use default: ~/.local/share/rustyssh/logs/
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustyssh")
            .join("logs")
    } else {
        configured.clone()
    }
}
