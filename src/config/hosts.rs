//! Host configuration structures

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::PathBuf;
use uuid::Uuid;

/// Reference to a jump host - can be UUID, hostname, or connection name
#[derive(Debug, Clone, PartialEq)]
pub enum JumpHostRef {
    /// Reference by UUID
    ByUuid(Uuid),
    /// Reference by hostname or IP address
    ByHostname(String),
    /// Reference by connection name
    ByName(String),
}

impl Serialize for JumpHostRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            JumpHostRef::ByUuid(uuid) => serializer.serialize_str(&uuid.to_string()),
            JumpHostRef::ByHostname(hostname) => serializer.serialize_str(hostname),
            JumpHostRef::ByName(name) => serializer.serialize_str(name),
        }
    }
}

impl<'de> Deserialize<'de> for JumpHostRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        // Try parsing as UUID first
        if let Ok(uuid) = Uuid::parse_str(&s) {
            return Ok(JumpHostRef::ByUuid(uuid));
        }
        // Otherwise treat as string (will be resolved as hostname or name later)
        Ok(JumpHostRef::ByHostname(s))
    }
}

/// Proxy configuration for reaching a host through various proxy mechanisms
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProxyConfig {
    /// Jump host (SSH ProxyJump) - tunnel through another SSH host
    JumpHost {
        /// Reference to the jump host (UUID, hostname, or connection name)
        host: JumpHostRef,
    },
    /// SOCKS5 proxy (RFC 1928)
    Socks5 {
        /// Proxy server address
        address: String,
        /// Proxy server port
        port: u16,
        /// Optional username for authentication
        #[serde(default, skip_serializing_if = "Option::is_none")]
        username: Option<String>,
        /// Optional password for authentication
        #[serde(default, skip_serializing_if = "Option::is_none")]
        password: Option<String>,
    },
    /// SOCKS4 proxy
    Socks4 {
        /// Proxy server address
        address: String,
        /// Proxy server port
        port: u16,
        /// Optional user ID
        #[serde(default, skip_serializing_if = "Option::is_none")]
        user_id: Option<String>,
    },
    /// HTTP CONNECT proxy
    Http {
        /// Proxy server address
        address: String,
        /// Proxy server port
        port: u16,
        /// Optional username for Basic authentication
        #[serde(default, skip_serializing_if = "Option::is_none")]
        username: Option<String>,
        /// Optional password for Basic authentication
        #[serde(default, skip_serializing_if = "Option::is_none")]
        password: Option<String>,
    },
    /// Custom proxy command (like OpenSSH ProxyCommand)
    /// The command is executed and stdin/stdout are used as the connection
    /// Supports %h (hostname) and %p (port) substitutions
    ProxyCommand {
        /// Command to execute (e.g., "nc %h %p" or "ncat --proxy proxy:8080 %h %p")
        command: String,
    },
}

/// Tunnel reference (by name or inline config)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TunnelRef {
    /// Reference by tunnel name
    Name(String),
    /// Inline tunnel configuration (legacy or convenience)
    Inline(TunnelConfig),
}

impl TunnelRef {
    /// Get the tunnel name from the reference
    pub fn name(&self) -> &str {
        match self {
            TunnelRef::Name(name) => name,
            TunnelRef::Inline(cfg) => cfg.name(),
        }
    }
}

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
    /// Proxy configuration (jump host, SOCKS, HTTP, or custom command)
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    /// Tags for organization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Configured tunnels
    #[serde(default)]
    pub tunnels: Vec<TunnelRef>,
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
    /// Remember password for this connection (encrypted with master password)
    #[serde(default)]
    pub remember_password: bool,
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
            proxy: None,
            tags: vec![],
            tunnels: vec![],
            startup_commands: vec![],
            environment: std::collections::HashMap::new(),
            notes: String::new(),
            color: None,
            remember_password: false,
        }
    }
}

impl HostConfig {
    /// Create a new host with basic info
    pub fn new(
        name: impl Into<String>,
        hostname: impl Into<String>,
        username: impl Into<String>,
    ) -> Self {
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl TunnelConfig {
    /// Get the tunnel name
    pub fn name(&self) -> &str {
        match self {
            TunnelConfig::Local { name, .. } => name,
            TunnelConfig::Remote { name, .. } => name,
            TunnelConfig::Dynamic { name, .. } => name,
        }
    }

    /// Get the auto-start flag
    pub fn auto_start(&self) -> bool {
        match self {
            TunnelConfig::Local { auto_start, .. } => *auto_start,
            TunnelConfig::Remote { auto_start, .. } => *auto_start,
            TunnelConfig::Dynamic { auto_start, .. } => *auto_start,
        }
    }

    /// Get a short type label
    pub fn type_label(&self) -> &'static str {
        match self {
            TunnelConfig::Local { .. } => "Local",
            TunnelConfig::Remote { .. } => "Remote",
            TunnelConfig::Dynamic { .. } => "Dynamic",
        }
    }

    /// Human-readable configuration summary
    pub fn description(&self) -> String {
        match self {
            TunnelConfig::Local {
                bind_addr,
                bind_port,
                remote_host,
                remote_port,
                ..
            } => format!("{}:{} → {}:{}", bind_addr, bind_port, remote_host, remote_port),
            TunnelConfig::Remote {
                remote_addr,
                remote_port,
                local_host,
                local_port,
                ..
            } => format!("{}:{} → {}:{}", remote_addr, remote_port, local_host, local_port),
            TunnelConfig::Dynamic {
                bind_addr,
                bind_port,
                ..
            } => format!("{}:{}", bind_addr, bind_port),
        }
    }
}
