//! Command Execution Integration Tests
//!
//! Tests for SSH connection features and configuration.
//! Tests marked with #[ignore] require Docker SSH servers to be running.

/// Test host config notes field (no server needed)
#[test]
fn test_host_config_notes() {
    use rustyssh::config::HostConfig;

    let mut host = HostConfig::new("server", "host.com", "user");
    host.notes = "Production server - handle with care".to_string();

    assert!(host.notes.contains("Production"));
}

/// Test host group organization (no server needed)
#[test]
fn test_host_group_creation() {
    use rustyssh::config::HostConfig;
    use rustyssh::config::HostGroup;

    let hosts = vec![
        HostConfig::new("web-1", "10.0.1.1", "admin"),
        HostConfig::new("web-2", "10.0.1.2", "admin"),
    ];

    let group = HostGroup {
        name: "Production".to_string(),
        hosts,
        expanded: true,
    };

    assert_eq!(group.name, "Production");
    assert_eq!(group.hosts.len(), 2);
    assert!(group.expanded);
}

/// Test tag-based organization (no server needed)
#[test]
fn test_host_tagging() {
    use rustyssh::config::HostConfig;

    let mut host = HostConfig::new("server", "host.com", "user");
    host.tags = vec![
        "production".to_string(),
        "web".to_string(),
        "frontend".to_string(),
    ];

    assert_eq!(host.tags.len(), 3);
    assert!(host.tags.contains(&"production".to_string()));
    assert!(host.tags.contains(&"web".to_string()));
}

/// Test environment variables configuration (no server needed)
#[test]
fn test_environment_config() {
    use rustyssh::config::HostConfig;
    use std::collections::HashMap;

    let mut env = HashMap::new();
    env.insert("TERM".to_string(), "xterm-256color".to_string());
    env.insert("LC_ALL".to_string(), "en_US.UTF-8".to_string());

    let mut host = HostConfig::new("server", "host.com", "user");
    host.environment = env;

    assert_eq!(
        host.environment.get("TERM"),
        Some(&"xterm-256color".to_string())
    );
}

/// Test startup commands configuration (no server needed)
#[test]
fn test_startup_commands() {
    use rustyssh::config::HostConfig;

    let mut host = HostConfig::new("server", "host.com", "user");
    host.startup_commands = vec!["cd /app".to_string(), "source .env".to_string()];

    assert_eq!(host.startup_commands.len(), 2);
}

/// Test jump host reference configuration (no server needed)
#[test]
fn test_jump_host_ref() {
    use rustyssh::config::{HostConfig, JumpHostRef, ProxyConfig};
    use uuid::Uuid;

    // Create a jump host reference by hostname
    let jump_ref = JumpHostRef::ByHostname("bastion.example.com".to_string());

    let mut host = HostConfig::new("internal-server", "10.0.0.1", "admin");
    host.proxy = Some(ProxyConfig::JumpHost { host: jump_ref });

    assert!(host.proxy.is_some());
    match host.proxy.as_ref().unwrap() {
        ProxyConfig::JumpHost {
            host: JumpHostRef::ByHostname(h),
        } => assert_eq!(h, "bastion.example.com"),
        _ => panic!("Expected JumpHost with ByHostname variant"),
    }

    // Test by UUID
    let id = Uuid::new_v4();
    let jump_by_uuid = JumpHostRef::ByUuid(id);
    match jump_by_uuid {
        JumpHostRef::ByUuid(u) => assert_eq!(u, id),
        _ => panic!("Expected ByUuid variant"),
    }

    // Test by name
    let jump_by_name = JumpHostRef::ByName("my-bastion".to_string());
    match jump_by_name {
        JumpHostRef::ByName(n) => assert_eq!(n, "my-bastion"),
        _ => panic!("Expected ByName variant"),
    }
}

/// Test Config loading and saving behavior (no server needed)
#[test]
fn test_config_default() {
    use rustyssh::config::Config;

    let config = Config::default();

    // Should have default groups
    assert!(!config.groups.is_empty());
    assert!(config.groups.iter().any(|g| g.name == "Production"));
    assert!(config.groups.iter().any(|g| g.name == "Development"));

    // Should have no hosts by default
    assert!(config.hosts.is_empty());
}

/// Test Config host operations (no server needed)
#[test]
fn test_config_add_host() {
    use rustyssh::config::{Config, HostConfig};

    let mut config = Config::default();

    // Add to ungrouped
    let host1 = HostConfig::new("server1", "host1.com", "user");
    config.add_host(host1, None);
    assert_eq!(config.hosts.len(), 1);

    // Add to Production group
    let host2 = HostConfig::new("server2", "host2.com", "user");
    config.add_host(host2, Some("Production"));

    // Verify
    let all = config.all_hosts();
    assert_eq!(all.len(), 2);
}

/// Test Config host finding (no server needed)
#[test]
fn test_config_find_host() {
    use rustyssh::config::{Config, HostConfig};
    use uuid::Uuid;

    let mut config = Config::default();

    let id = Uuid::new_v4();
    let mut host = HostConfig::new("findme", "find.com", "user");
    host.id = id;

    config.add_host(host, None);

    // Find by ID
    let found = config.find_host(id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "findme");

    // Find non-existent
    let not_found = config.find_host(Uuid::new_v4());
    assert!(not_found.is_none());
}
