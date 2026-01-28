//! Connection Integration Tests
//!
//! Tests for SSH connection lifecycle including connecting, disconnecting,
//! and managing multiple connections.
//!
//! Tests marked with #[ignore] require Docker SSH servers to be running.

use crate::common::*;
use rustyssh::ssh::SshConnection;
use std::collections::HashMap;

/// Test connection to Docker test server (requires Docker)
#[tokio::test]
#[ignore = "Requires Docker SSH servers - run ./tests/docker/start_servers.sh first"]
async fn test_connect_to_docker_server() {
    let host_config = create_docker_host_config_1();

    let result = SshConnection::connect(host_config, Some(TEST_PASSWORD), None);

    match result {
        Ok(conn) => {
            assert!(conn.is_alive());
            println!("Successfully connected to Docker SSH server");
            let _ = conn.close();
        }
        Err(e) => {
            panic!("Connection failed: {}", e);
        }
    }
}

/// Test connection to non-existent server fails gracefully
#[tokio::test]
async fn test_connection_to_invalid_host() {
    use rustyssh::config::{AuthMethod, HostConfig};
    use uuid::Uuid;

    let host_config = HostConfig {
        id: Uuid::new_v4(),
        name: "nonexistent".to_string(),
        hostname: "127.0.0.1".to_string(),
        port: 59999, // Unlikely to be in use
        username: "user".to_string(),
        auth: AuthMethod::Password,
        tags: vec![],
        proxy: None,
        notes: String::new(),
        tunnels: vec![],
        startup_commands: vec![],
        environment: HashMap::new(),
        color: None,
        remember_password: false,
    };

    let result = SshConnection::connect(host_config, Some("pass"), None);
    assert!(result.is_err(), "Should fail to connect to non-existent server");
}

/// Test multiple parallel connections to different Docker servers (requires Docker)
#[tokio::test]
#[ignore = "Requires Docker SSH servers - run ./tests/docker/start_servers.sh first"]
async fn test_multiple_parallel_connections() {
    let host1 = create_docker_host_config_1();
    let host2 = create_docker_host_config_2();

    let conn1 = SshConnection::connect(host1, Some(TEST_PASSWORD), None);
    let conn2 = SshConnection::connect(host2, Some(TEST_PASSWORD), None);

    assert!(conn1.is_ok(), "Should connect to server 1");
    assert!(conn2.is_ok(), "Should connect to server 2");

    if let (Ok(c1), Ok(c2)) = (conn1, conn2) {
        assert!(c1.is_alive());
        assert!(c2.is_alive());
        let _ = c1.close();
        let _ = c2.close();
    }
}

/// Test connection pool functionality (no server needed)
#[tokio::test]
async fn test_connection_pool() {
    use rustyssh::ssh::ConnectionPool;
    use uuid::Uuid;

    let mut pool = ConnectionPool::new();

    // Pool should start empty
    assert_eq!(pool.count(), 0);
    assert!(pool.list().is_empty());

    // Get non-existent connection
    let fake_id = Uuid::new_v4();
    assert!(pool.get(fake_id).is_none());
    assert!(pool.get_mut(fake_id).is_none());

    // Remove non-existent connection
    assert!(pool.remove(fake_id).is_none());
}

/// Test command execution on Docker server (requires Docker)
#[tokio::test]
#[ignore = "Requires Docker SSH servers - run ./tests/docker/start_servers.sh first"]
async fn test_exec_command() {
    let host_config = create_docker_host_config_1();

    let conn = SshConnection::connect(host_config, Some(TEST_PASSWORD), None)
        .expect("Should connect");

    let output = conn.exec("echo hello").expect("Should execute command");
    assert!(output.contains("hello"), "Output should contain 'hello'");

    let _ = conn.close();
}

/// Test Docker servers check utility
#[tokio::test]
async fn test_docker_servers_check() {
    let available = docker_servers_available().await;
    println!("Docker servers available: {}", available);
    // This test always passes - it's informational
}
