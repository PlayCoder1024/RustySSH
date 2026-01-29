//! Authentication Integration Tests
//!
//! Tests for SSH authentication methods including password and key-based auth.
//!
//! Tests marked with #[ignore] require Docker SSH servers to be running.

use crate::common::*;
use rustyssh::ssh::SshConnection;
use std::collections::HashMap;

/// Test successful password authentication (requires Docker)
#[tokio::test]
#[ignore = "Requires Docker SSH servers - run ./tests/docker/start_servers.sh first"]
async fn test_password_auth_success() {
    let host_config = create_docker_host_config_1();

    let result = SshConnection::connect(host_config, Some(TEST_PASSWORD), None);

    assert!(result.is_ok(), "Password auth should succeed");
    if let Ok(conn) = result {
        assert!(conn.is_alive());
        let _ = conn.close();
    }
}

/// Test password authentication with wrong password fails (requires Docker)
#[tokio::test]
#[ignore = "Requires Docker SSH servers - run ./tests/docker/start_servers.sh first"]
async fn test_password_auth_failure() {
    let host_config = create_docker_host_config_1();

    let result = SshConnection::connect(host_config, Some(INVALID_PASSWORD), None);

    assert!(result.is_err(), "Wrong password should fail");
}

/// Test authentication with different users (requires Docker)
#[tokio::test]
#[ignore = "Requires Docker SSH servers - run ./tests/docker/start_servers.sh first"]
async fn test_multi_user_auth() {
    // Server 1 with testuser
    let host1 = create_docker_host_config_1();
    let result1 = SshConnection::connect(host1, Some(TEST_PASSWORD), None);
    assert!(result1.is_ok(), "Should connect as testuser");
    if let Ok(conn) = result1 {
        let _ = conn.close();
    }

    // Server 3 with altuser
    let host3 = create_docker_host_config_3();
    let result3 = SshConnection::connect(host3, Some(ALT_PASSWORD), None);
    assert!(result3.is_ok(), "Should connect as altuser");
    if let Ok(conn) = result3 {
        let _ = conn.close();
    }
}

/// Test that authentication without password fails for password-required auth
#[tokio::test]
async fn test_password_auth_no_password_provided() {
    let host_config = create_docker_host_config_1();

    // Don't provide password
    let result = SshConnection::connect(host_config, None, None);

    // Should fail - password required but not provided
    assert!(
        result.is_err(),
        "Should fail when password not provided for password auth"
    );
}

/// Test Authenticator unit tests (no server needed)
#[test]
fn test_auth_method_variants() {
    use rustyssh::config::AuthMethod;
    use std::path::PathBuf;

    // Password auth
    let password_auth = AuthMethod::Password;
    assert!(matches!(password_auth, AuthMethod::Password));

    // Key file auth
    let key_auth = AuthMethod::KeyFile {
        path: PathBuf::from("/path/to/key"),
        passphrase_required: true,
    };
    assert!(matches!(key_auth, AuthMethod::KeyFile { .. }));

    // Agent auth
    let agent_auth = AuthMethod::Agent;
    assert!(matches!(agent_auth, AuthMethod::Agent));

    // Certificate auth
    let cert_auth = AuthMethod::Certificate {
        cert_path: PathBuf::from("/path/to/cert"),
        key_path: PathBuf::from("/path/to/key"),
    };
    assert!(matches!(cert_auth, AuthMethod::Certificate { .. }));
}

/// Test HostConfig construction (no server needed)
#[test]
fn test_host_config_creation() {
    use rustyssh::config::HostConfig;

    // Test default
    let default = HostConfig::default();
    assert_eq!(default.port, 22);
    assert!(matches!(
        default.auth,
        rustyssh::config::AuthMethod::Password
    ));

    // Test new constructor
    let host = HostConfig::new("my-server", "192.168.1.100", "admin");
    assert_eq!(host.name, "my-server");
    assert_eq!(host.hostname, "192.168.1.100");
    assert_eq!(host.username, "admin");
    assert_eq!(host.port, 22);
}

/// Test connection string formatting (no server needed)
#[test]
fn test_connection_string() {
    use rustyssh::config::HostConfig;

    let host = HostConfig::new("server", "example.com", "user");
    assert_eq!(host.connection_string(), "user@example.com:22");
}
