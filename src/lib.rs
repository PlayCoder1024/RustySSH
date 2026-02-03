//! RustySSH - A high-performance TUI SSH connection manager
//!
//! This crate provides a terminal-based SSH client with features including:
//! - SSH connection management with multiple authentication methods
//! - Interactive terminal sessions with VT100 emulation
//! - SSH tunneling (local, remote, dynamic)
//! - SFTP file browser with dual-pane interface

pub mod app;
pub mod config;
pub mod credentials;
pub mod logging;
pub mod sftp;
pub mod ssh;
pub mod tui;
pub mod utils;

pub use app::App;
pub use config::Config;
