//! RustySSH Integration Tests
//!
//! This test suite includes:
//! - Unit tests that don't require real SSH connections
//! - Integration tests that require Docker SSH servers (marked with #[ignore])
//!
//! # Running Tests
//!
//! ## Unit tests only (no setup required):
//! ```bash
//! cargo test --test main
//! ```
//!
//! ## Integration tests (requires Docker):
//! ```bash
//! # Start test servers
//! ./tests/docker/start_servers.sh
//!
//! # Run all tests including integration
//! cargo test --test main -- --include-ignored
//!
//! # Stop servers when done
//! ./tests/docker/start_servers.sh stop
//! ```

mod common;

// Include integration test modules
mod integration {
    pub mod auth;
    pub mod command;
    pub mod connection;
    pub mod session;
}

// Re-export for convenience in tests
pub use common::*;
