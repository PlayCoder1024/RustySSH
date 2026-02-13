//! SSH functionality
//!
//! This module uses the ssh2 crate which wraps libssh2

mod auth;
mod connection;
mod keys;
mod session;
mod tunnel;

pub use auth::Authenticator;
pub use connection::{ConnectionPool, ProxyConnection, SshConnection};
pub use keys::KeyManager;
pub use session::{Session, SessionManager, SessionStatus, TextSelection};
pub use tunnel::{run_tunnel, Tunnel, TunnelManager, TunnelType};
