//! SSH connection management

use crate::config::HostConfig;
use anyhow::{anyhow, Result};
use ssh2::Session;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use uuid::Uuid;

/// Active SSH connection
pub struct SshConnection {
    /// Connection ID
    pub id: Uuid,
    /// Host configuration
    pub host: HostConfig,
    /// SSH session
    session: Session,
    /// TCP stream (kept alive for the session)
    _stream: TcpStream,
    /// Connection status
    pub status: ConnectionStatus,
}

/// Connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Authenticated,
    Disconnected,
}

impl SshConnection {
    /// Create a new connection to a host
    pub fn connect(host: HostConfig, password: Option<&str>, passphrase: Option<&str>) -> Result<Self> {
        // Connect TCP
        let addr = format!("{}:{}", host.hostname, host.port);
        let stream = TcpStream::connect(&addr)
            .map_err(|e| anyhow!("Failed to connect to {}: {}", addr, e))?;
        
        // Set non-blocking mode for later async operations
        stream.set_nonblocking(false)?;
        
        // Create SSH session
        let mut session = Session::new()
            .map_err(|e| anyhow!("Failed to create SSH session: {}", e))?;
        
        session.set_tcp_stream(stream.try_clone()?);
        
        // SSH handshake
        session.handshake()
            .map_err(|e| anyhow!("SSH handshake failed: {}", e))?;
        
        // Authenticate
        crate::ssh::auth::Authenticator::authenticate_any(
            &session,
            &host,
            password,
            passphrase,
        )?;

        if !session.authenticated() {
            return Err(anyhow!("Authentication failed"));
        }

        Ok(Self {
            id: Uuid::new_v4(),
            host,
            session,
            _stream: stream,
            status: ConnectionStatus::Authenticated,
        })
    }

    /// Get a reference to the SSH session
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Open an interactive shell channel
    pub fn open_shell(&mut self, cols: u32, rows: u32) -> Result<ssh2::Channel> {
        let mut channel = self.session.channel_session()
            .map_err(|e| anyhow!("Failed to open channel: {}", e))?;
        
        // Request PTY
        channel.request_pty("xterm-256color", None, Some((cols, rows, 0, 0)))
            .map_err(|e| anyhow!("Failed to request PTY: {}", e))?;

        // Request shell
        channel.shell()
            .map_err(|e| anyhow!("Failed to request shell: {}", e))?;

        // Set session to non-blocking for async I/O
        self.session.set_blocking(false);

        Ok(channel)
    }

    /// Open a direct TCP/IP channel (for port forwarding)
    pub fn open_direct_tcpip(&self, host: &str, port: u16) -> Result<ssh2::Channel> {
        self.session.channel_direct_tcpip(host, port, None)
            .map_err(|e| anyhow!("Failed to open direct-tcpip channel: {}", e))
    }

    /// Open SFTP subsystem
    pub fn open_sftp(&self) -> Result<ssh2::Sftp> {
        self.session.sftp()
            .map_err(|e| anyhow!("Failed to open SFTP: {}", e))
    }

    /// Check if connection is alive
    pub fn is_alive(&self) -> bool {
        self.session.authenticated() && self.status == ConnectionStatus::Authenticated
    }

    /// Execute a command and return output
    pub fn exec(&self, command: &str) -> Result<String> {
        let mut channel = self.session.channel_session()
            .map_err(|e| anyhow!("Failed to open channel: {}", e))?;
        
        channel.exec(command)
            .map_err(|e| anyhow!("Failed to execute command: {}", e))?;
        
        let mut output = String::new();
        channel.read_to_string(&mut output)?;
        
        channel.wait_close()?;
        
        Ok(output)
    }

    /// Close the connection
    pub fn close(self) -> Result<()> {
        self.session.disconnect(None, "Goodbye", None)
            .map_err(|e| anyhow!("Failed to disconnect: {}", e))
    }
}

/// Connection pool for managing multiple connections
pub struct ConnectionPool {
    connections: HashMap<Uuid, SshConnection>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    /// Add a connection to the pool
    pub fn add(&mut self, connection: SshConnection) -> Uuid {
        let id = connection.id;
        self.connections.insert(id, connection);
        id
    }

    /// Get a connection by ID
    pub fn get(&self, id: Uuid) -> Option<&SshConnection> {
        self.connections.get(&id)
    }

    /// Get a mutable connection by ID
    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut SshConnection> {
        self.connections.get_mut(&id)
    }

    /// Remove a connection from the pool
    pub fn remove(&mut self, id: Uuid) -> Option<SshConnection> {
        self.connections.remove(&id)
    }

    /// List all connection IDs
    pub fn list(&self) -> Vec<Uuid> {
        self.connections.keys().copied().collect()
    }

    /// Get connection count
    pub fn count(&self) -> usize {
        self.connections.len()
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}
