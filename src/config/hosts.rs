//! Host configuration structures

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// SSH host configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    /// Unique identifier
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    /// Display name
    pub name: String,
    /// Hostname or IP address
    pub hostname: String,
    /// SSH port (default: 22)
    #[serde(default = "default_port")]
    pub port: u16,
    /// Username
    pub username: String,
    /// Authentication method
    #[serde(default)]
    pub auth: AuthMethod,
    /// Jump host (ProxyJump) - ID of another host
    pub jump_host: Option<Uuid>,
    /// Tags for organization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Configured tunnels
    #[serde(default)]
    pub tunnels: Vec<TunnelConfig>,
    /// Commands to run on connect
    #[serde(default)]
    pub startup_commands: Vec<String>,
    /// Custom environment variables
    #[serde(default)]
    pub environment: std::collections::HashMap<String, String>,
    /// Notes/description
    #[serde(default)]
    pub notes: String,
    /// Color for visual identification (hex)
    pub color: Option<String>,
}

fn default_port() -> u16 {
    22
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            hostname: String::new(),
            port: 22,
            username: whoami::username(),
            auth: AuthMethod::default(),
            jump_host: None,
            tags: vec![],
            tunnels: vec![],
            startup_commands: vec![],
            environment: std::collections::HashMap::new(),
            notes: String::new(),
            color: None,
        }
    }
}

impl HostConfig {
    /// Create a new host with basic info
    pub fn new(name: impl Into<String>, hostname: impl Into<String>, username: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            hostname: hostname.into(),
            username: username.into(),
            ..Default::default()
        }
    }

    /// Get display string for connection
    pub fn connection_string(&self) -> String {
        format!("{}@{}:{}", self.username, self.hostname, self.port)
    }
}

/// Authentication method
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthMethod {
    /// Password authentication (prompt at connection time)
    #[default]
    Password,
    /// Private key file
    KeyFile {
        /// Path to private key
        path: PathBuf,
        /// Whether key requires passphrase
        #[serde(default)]
        passphrase_required: bool,
    },
    /// SSH agent
    Agent,
    /// Certificate authentication
    Certificate {
        /// Path to certificate
        cert_path: PathBuf,
        /// Path to private key
        key_path: PathBuf,
    },
}

/// Host group for organization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostGroup {
    /// Group name
    pub name: String,
    /// Hosts in this group
    #[serde(default)]
    pub hosts: Vec<HostConfig>,
    /// Whether group is expanded in UI
    #[serde(default = "default_true")]
    pub expanded: bool,
}

fn default_true() -> bool {
    true
}

/// Tunnel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TunnelConfig {
    /// Local port forwarding (-L)
    Local {
        /// Name for this tunnel
        name: String,
        /// Local bind address
        bind_addr: String,
        /// Local bind port
        bind_port: u16,
        /// Remote host
        remote_host: String,
        /// Remote port
        remote_port: u16,
        /// Auto-start on connect
        #[serde(default)]
        auto_start: bool,
    },
    /// Remote port forwarding (-R)
    Remote {
        /// Name for this tunnel
        name: String,
        /// Remote bind address
        remote_addr: String,
        /// Remote bind port
        remote_port: u16,
        /// Local host
        local_host: String,
        /// Local port
        local_port: u16,
        /// Auto-start on connect
        #[serde(default)]
        auto_start: bool,
    },
    /// Dynamic SOCKS proxy (-D)
    Dynamic {
        /// Name for this tunnel
        name: String,
        /// Local bind address
        bind_addr: String,
        /// Local bind port
        bind_port: u16,
        /// Auto-start on connect
        #[serde(default)]
        auto_start: bool,
    },
}
