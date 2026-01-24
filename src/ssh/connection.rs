//! SSH connection management

use crate::config::HostConfig;
use anyhow::{anyhow, Result};
use ssh2::Session;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use uuid::Uuid;

/// Proxy connection type - extensible for future proxy types
pub enum ProxyConnection {
    /// Direct connection (no proxy)
    Direct,
    /// SSH jump host connection - tunnels through an existing SSH connection
    JumpHost {
        /// The SSH connection to tunnel through
        connection: Box<SshConnection>,
    },
    // Future proxy types can be added here:
    // Socks5 { addr: String, port: u16, auth: Option<(String, String)> },
    // HttpProxy { addr: String, port: u16, auth: Option<(String, String)> },
}



/// Active SSH connection
pub struct SshConnection {
    /// Connection ID
    pub id: Uuid,
    /// Host configuration
    pub host: HostConfig,
    /// SSH session
    session: Session,
    /// TCP stream (kept alive for direct connections)
    _stream: Option<TcpStream>,
    /// Jump host connection (kept alive for proxied connections)
    _jump_connection: Option<Box<SshConnection>>,
    /// Tunnel channel (kept alive for proxied connections)
    _tunnel_channel: Option<ssh2::Channel>,
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
    /// Create a new direct connection to a host (no proxy)
    pub fn connect(host: HostConfig, password: Option<&str>, passphrase: Option<&str>) -> Result<Self> {
        Self::connect_via_proxy(host, ProxyConnection::Direct, password, passphrase)
    }

    /// Create a connection through a proxy
    pub fn connect_via_proxy(
        host: HostConfig,
        proxy: ProxyConnection,
        password: Option<&str>,
        passphrase: Option<&str>,
    ) -> Result<Self> {
        // Connection timeout
        const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
        
        match proxy {
            ProxyConnection::Direct => {
                // Direct TCP connection with timeout
                let addr = format!("{}:{}", host.hostname, host.port);
                let socket_addr = addr.to_socket_addrs()
                    .map_err(|e| anyhow!("Failed to resolve {}: {}", addr, e))?
                    .next()
                    .ok_or_else(|| anyhow!("No addresses found for {}", addr))?;
                
                let stream = TcpStream::connect_timeout(&socket_addr, CONNECT_TIMEOUT)
                    .map_err(|e| anyhow!("Connection to {} timed out or failed: {}", addr, e))?;
                
                stream.set_nonblocking(false)?;
                
                let mut session = Session::new()
                    .map_err(|e| anyhow!("Failed to create SSH session: {}", e))?;
                
                session.set_tcp_stream(stream.try_clone()?);
                
                session.handshake()
                    .map_err(|e| anyhow!("SSH handshake failed: {}", e))?;
                
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
                    _stream: Some(stream),
                    _jump_connection: None,
                    _tunnel_channel: None,
                    status: ConnectionStatus::Authenticated,
                })
            }
            ProxyConnection::JumpHost { connection } => {
                // Connect through jump host using direct-tcpip channel
                let addr = format!("{}:{}", host.hostname, host.port);
                
                // Ensure the jump connection session is in blocking mode for channel setup
                connection.session.set_blocking(true);
                
                // Open a direct-tcpip channel through the jump host
                let channel = connection.session.channel_direct_tcpip(
                    &host.hostname,
                    host.port,
                    None, // source addr not needed
                ).map_err(|e| anyhow!("Failed to open tunnel through jump host to {}: {}", addr, e))?;
                
                // Set session back to non-blocking mode for the proxy thread
                // This is critical! Without this, channel.read() will block forever
                connection.session.set_blocking(false);
                
                // Create a new SSH session and use the channel as transport
                // Note: ssh2 requires a TcpStream, so we use a socket pair approach
                // Actually, ssh2's Session can use any Read+Write stream via set_tcp_stream
                // But set_tcp_stream only accepts TcpStream, so we need a workaround
                //
                // Workaround: Create a local socket pair and proxy the channel through it
                let (local_stream, remote_stream) = Self::create_socket_pair()?;
                
                // Spawn a thread to proxy between the channel and the socket
                let channel_clone = channel;
                std::thread::spawn(move || {
                    Self::proxy_channel_to_stream(channel_clone, remote_stream);
                });
                
                // Now create SSH session over the local end of the socket pair
                let mut session = Session::new()
                    .map_err(|e| anyhow!("Failed to create SSH session: {}", e))?;
                
                session.set_tcp_stream(local_stream.try_clone()?);
                
                session.handshake()
                    .map_err(|e| anyhow!("SSH handshake through jump host failed: {}", e))?;
                
                crate::ssh::auth::Authenticator::authenticate_any(
                    &session,
                    &host,
                    password,
                    passphrase,
                )?;

                if !session.authenticated() {
                    return Err(anyhow!("Authentication through jump host failed"));
                }

                Ok(Self {
                    id: Uuid::new_v4(),
                    host,
                    session,
                    _stream: Some(local_stream),
                    _jump_connection: Some(connection),
                    _tunnel_channel: None,
                    status: ConnectionStatus::Authenticated,
                })
            }
        }
    }

    /// Create a socket pair for proxying
    fn create_socket_pair() -> Result<(TcpStream, TcpStream)> {
        use std::net::TcpListener;
        
        // Bind to localhost on a random port
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        
        // Connect to it (localhost, no timeout needed)
        let client = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
        
        // Accept the connection
        let (server, _) = listener.accept()?;
        
        Ok((client, server))
    }

    /// Proxy data between a channel and a stream
    fn proxy_channel_to_stream(mut channel: ssh2::Channel, mut stream: TcpStream) {
        use std::io::{ErrorKind, Read, Write};
        
        // Set stream to non-blocking
        let _ = stream.set_nonblocking(true);
        
        let mut buf = [0u8; 8192];
        
        loop {
            // Check if channel is closed
            if channel.eof() {
                break;
            }

            // Read from channel, write to stream
            match channel.read(&mut buf) {
                Ok(0) => {}
                Ok(n) => {
                    if stream.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    let _ = stream.flush();
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(_) => break,
            }

            // Read from stream, write to channel
            match stream.read(&mut buf) {
                Ok(0) => break, // Stream closed
                Ok(n) => {
                    if channel.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    let _ = channel.flush();
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(_) => break,
            }

            // Small sleep to avoid busy-looping
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
        
        let _ = channel.close();
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
    /// Note: The session stays in blocking mode for SFTP operations
    /// SFTP connections should be dedicated (separate from shell connections)
    pub fn open_sftp(&self) -> Result<ssh2::Sftp> {
        // SFTP operations require blocking mode
        // Since SFTP sessions should use dedicated connections (not shared with shell),
        // we keep the session in blocking mode for all SFTP operations to work correctly
        self.session.set_blocking(true);
        
        let sftp = self.session.sftp()
            .map_err(|e| anyhow!("Failed to open SFTP: {}", e))?;
        
        // Keep in blocking mode - SFTP operations (readdir, create, read, write)
        // all require blocking mode to work correctly
        
        Ok(sftp)
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
