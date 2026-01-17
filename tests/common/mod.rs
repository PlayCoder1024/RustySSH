//! Common Test Utilities
//!
//! Helper functions and utilities shared across integration tests.

pub mod fixtures;

pub use fixtures::*;

use rustyssh::config::{AuthMethod, HostConfig};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use uuid::Uuid;

/// Default Docker test server address
pub const DOCKER_SERVER_1: &str = "127.0.0.1:2222";
pub const DOCKER_SERVER_2: &str = "127.0.0.1:2223";
pub const DOCKER_SERVER_3: &str = "127.0.0.1:2224";

/// Check if Docker test servers are available
pub async fn docker_servers_available() -> bool {
    let addr: SocketAddr = DOCKER_SERVER_1.parse().unwrap();
    timeout(Duration::from_secs(1), TcpStream::connect(addr))
        .await
        .is_ok()
}

/// Wait for a port to become available
pub async fn wait_for_port(addr: &str, max_wait: Duration) -> anyhow::Result<()> {
    let addr: SocketAddr = addr.parse()?;
    let start = std::time::Instant::now();

    while start.elapsed() < max_wait {
        match timeout(Duration::from_millis(100), TcpStream::connect(addr)).await {
            Ok(Ok(_)) => return Ok(()),
            _ => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }

    anyhow::bail!("Port {} not available after {:?}", addr, max_wait)
}

/// Create a HostConfig for Docker test server 1
pub fn create_docker_host_config_1() -> HostConfig {
    HostConfig {
        id: Uuid::new_v4(),
        name: "docker-test-1".to_string(),
        hostname: "127.0.0.1".to_string(),
        port: 2222,
        username: TEST_USERNAME.to_string(),
        auth: AuthMethod::Password,
        tags: vec!["test".to_string()],
        jump_host: None,
        notes: String::new(),
        tunnels: vec![],
        startup_commands: vec![],
        environment: HashMap::new(),
        color: None,
        remember_password: false,
    }
}

/// Create a HostConfig for Docker test server 2
pub fn create_docker_host_config_2() -> HostConfig {
    HostConfig {
        id: Uuid::new_v4(),
        name: "docker-test-2".to_string(),
        hostname: "127.0.0.1".to_string(),
        port: 2223,
        username: TEST_USERNAME.to_string(),
        auth: AuthMethod::Password,
        tags: vec!["test".to_string()],
        jump_host: None,
        notes: String::new(),
        tunnels: vec![],
        startup_commands: vec![],
        environment: HashMap::new(),
        color: None,
        remember_password: false,
    }
}

/// Create a HostConfig for Docker test server 3 (different user)
pub fn create_docker_host_config_3() -> HostConfig {
    HostConfig {
        id: Uuid::new_v4(),
        name: "docker-test-3".to_string(),
        hostname: "127.0.0.1".to_string(),
        port: 2224,
        username: ALT_USERNAME.to_string(),
        auth: AuthMethod::Password,
        tags: vec!["test".to_string()],
        jump_host: None,
        notes: String::new(),
        tunnels: vec![],
        startup_commands: vec![],
        environment: HashMap::new(),
        color: None,
        remember_password: false,
    }
}

/// Assert that a connection succeeded
#[macro_export]
macro_rules! assert_connected {
    ($result:expr) => {
        match $result {
            Ok(conn) => conn,
            Err(e) => panic!("Connection failed: {}", e),
        }
    };
}

/// Assert that a connection failed with an error
#[macro_export]
macro_rules! assert_connection_failed {
    ($result:expr) => {
        assert!(
            $result.is_err(),
            "Expected connection to fail, but it succeeded"
        )
    };
}
