//! SSH connection management

use crate::config::HostConfig;
use anyhow::{anyhow, Result};
use ssh2::Session;
use std::collections::HashMap;
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use tracing::{debug, error, info};

use uuid::Uuid;

/// Proxy connection type for reaching SSH hosts through various mechanisms
pub enum ProxyConnection {
    /// Direct connection (no proxy)
    Direct,
    /// SSH jump host connection - tunnels through an existing SSH connection
    JumpHost {
        /// The SSH connection to tunnel through
        connection: Box<SshConnection>,
    },
    /// SOCKS5 proxy (RFC 1928)
    Socks5 {
        /// Proxy server address
        address: String,
        /// Proxy server port
        port: u16,
        /// Optional username and password for authentication
        auth: Option<(String, String)>,
    },
    /// SOCKS4 proxy
    Socks4 {
        /// Proxy server address
        address: String,
        /// Proxy server port
        port: u16,
        /// Optional user ID
        user_id: Option<String>,
    },
    /// HTTP CONNECT proxy
    HttpConnect {
        /// Proxy server address
        address: String,
        /// Proxy server port
        port: u16,
        /// Optional username and password for Basic auth
        auth: Option<(String, String)>,
    },
    /// Custom proxy command (like OpenSSH ProxyCommand)
    /// The command is executed and stdin/stdout are used as the socket
    ProxyCommand {
        /// Command template with %h and %p substitutions
        command: String,
        /// Target hostname (for %h substitution)
        target_host: String,
        /// Target port (for %p substitution)
        target_port: u16,
    },
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
    /// Get the underlying SSH session
    pub fn session_ref(&self) -> &Session {
        &self.session
    }

    /// Create a new direct connection to a host (no proxy)
    pub fn connect(
        host: HostConfig,
        password: Option<&str>,
        passphrase: Option<&str>,
    ) -> Result<Self> {
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
                info!(target: "ssh", "Connecting directly to {}", addr);

                debug!(target: "ssh", "Resolving DNS for {}", host.hostname);
                let socket_addr = addr
                    .to_socket_addrs()
                    .map_err(|e| anyhow!("Failed to resolve {}: {}", addr, e))?
                    .next()
                    .ok_or_else(|| anyhow!("No addresses found for {}", addr))?;
                debug!(target: "ssh", "Resolved {} to {}", host.hostname, socket_addr);

                debug!(target: "ssh", "Establishing TCP connection to {}", socket_addr);
                let stream = TcpStream::connect_timeout(&socket_addr, CONNECT_TIMEOUT)
                    .map_err(|e| anyhow!("Connection to {} timed out or failed: {}", addr, e))?;

                stream.set_nonblocking(false)?;

                let mut session =
                    Session::new().map_err(|e| anyhow!("Failed to create SSH session: {}", e))?;

                session.set_tcp_stream(stream.try_clone()?);

                debug!(target: "ssh", "Performing SSH handshake with {}", host.hostname);
                session
                    .handshake()
                    .map_err(|e| anyhow!("SSH handshake failed: {}", e))?;
                debug!(target: "ssh", "SSH handshake completed with {}", host.hostname);

                crate::ssh::auth::Authenticator::authenticate_any(
                    &session, &host, password, passphrase,
                )?;

                if !session.authenticated() {
                    error!(target: "ssh", "Authentication failed for {}@{}", host.username, host.hostname);
                    return Err(anyhow!("Authentication failed"));
                }

                info!(target: "ssh", "Successfully connected to {}@{}", host.username, host.hostname);
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
                info!(target: "ssh", "Connecting to {} via jump host {}", addr, connection.host.hostname);

                // Ensure the jump connection session is in blocking mode for channel setup
                connection.session.set_blocking(true);

                // Open a direct-tcpip channel through the jump host
                debug!(target: "ssh", "Opening direct-tcpip channel through jump host to {}", addr);
                let channel = connection
                    .session
                    .channel_direct_tcpip(
                        &host.hostname,
                        host.port,
                        None, // source addr not needed
                    )
                    .map_err(|e| {
                        anyhow!("Failed to open tunnel through jump host to {}: {}", addr, e)
                    })?;
                debug!(target: "ssh", "Direct-tcpip channel opened to {}", addr);

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
                let mut session =
                    Session::new().map_err(|e| anyhow!("Failed to create SSH session: {}", e))?;

                session.set_tcp_stream(local_stream.try_clone()?);

                debug!(target: "ssh", "Performing SSH handshake through jump host to {}", host.hostname);
                session
                    .handshake()
                    .map_err(|e| anyhow!("SSH handshake through jump host failed: {}", e))?;

                crate::ssh::auth::Authenticator::authenticate_any(
                    &session, &host, password, passphrase,
                )?;

                if !session.authenticated() {
                    error!(target: "ssh", "Authentication through jump host failed for {}@{}", host.username, host.hostname);
                    return Err(anyhow!("Authentication through jump host failed"));
                }

                info!(target: "ssh", "Successfully connected to {}@{} via jump host", host.username, host.hostname);
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
            ProxyConnection::Socks5 {
                address,
                port,
                auth,
            } => {
                // Connect to SOCKS5 proxy
                let proxy_addr = format!("{}:{}", address, port);
                let target_addr = format!("{}:{}", host.hostname, host.port);
                info!(target: "ssh", "Connecting to {} via SOCKS5 proxy {}", target_addr, proxy_addr);

                let socket_addr = proxy_addr
                    .to_socket_addrs()
                    .map_err(|e| anyhow!("Failed to resolve SOCKS5 proxy {}: {}", proxy_addr, e))?
                    .next()
                    .ok_or_else(|| anyhow!("No addresses found for SOCKS5 proxy {}", proxy_addr))?;

                let mut stream = TcpStream::connect_timeout(&socket_addr, CONNECT_TIMEOUT)
                    .map_err(|e| {
                        anyhow!("Connection to SOCKS5 proxy {} failed: {}", proxy_addr, e)
                    })?;

                // SOCKS5 handshake (RFC 1928)
                debug!(target: "ssh", "Performing SOCKS5 handshake with proxy {}", proxy_addr);
                Self::socks5_handshake(&mut stream, &host.hostname, host.port, auth.as_ref())?;
                debug!(target: "ssh", "SOCKS5 handshake completed, tunnel established to {}", target_addr);

                stream.set_nonblocking(false)?;

                let mut session =
                    Session::new().map_err(|e| anyhow!("Failed to create SSH session: {}", e))?;

                session.set_tcp_stream(stream.try_clone()?);

                debug!(target: "ssh", "Performing SSH handshake through SOCKS5 proxy to {}", host.hostname);
                session
                    .handshake()
                    .map_err(|e| anyhow!("SSH handshake through SOCKS5 proxy failed: {}", e))?;

                crate::ssh::auth::Authenticator::authenticate_any(
                    &session, &host, password, passphrase,
                )?;

                if !session.authenticated() {
                    error!(target: "ssh", "Authentication through SOCKS5 proxy failed for {}@{}", host.username, host.hostname);
                    return Err(anyhow!("Authentication through SOCKS5 proxy failed"));
                }

                info!(target: "ssh", "Successfully connected to {}@{} via SOCKS5 proxy", host.username, host.hostname);
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
            ProxyConnection::Socks4 {
                address,
                port,
                user_id,
            } => {
                // Connect to SOCKS4 proxy
                let proxy_addr = format!("{}:{}", address, port);
                let target_addr = format!("{}:{}", host.hostname, host.port);
                info!(target: "ssh", "Connecting to {} via SOCKS4 proxy {}", target_addr, proxy_addr);

                let socket_addr = proxy_addr
                    .to_socket_addrs()
                    .map_err(|e| anyhow!("Failed to resolve SOCKS4 proxy {}: {}", proxy_addr, e))?
                    .next()
                    .ok_or_else(|| anyhow!("No addresses found for SOCKS4 proxy {}", proxy_addr))?;

                let mut stream = TcpStream::connect_timeout(&socket_addr, CONNECT_TIMEOUT)
                    .map_err(|e| {
                        anyhow!("Connection to SOCKS4 proxy {} failed: {}", proxy_addr, e)
                    })?;

                // SOCKS4 connect request
                debug!(target: "ssh", "Performing SOCKS4 handshake with proxy {}", proxy_addr);
                Self::socks4_handshake(&mut stream, &host.hostname, host.port, user_id.as_deref())?;
                debug!(target: "ssh", "SOCKS4 handshake completed, tunnel established to {}", target_addr);

                stream.set_nonblocking(false)?;

                let mut session =
                    Session::new().map_err(|e| anyhow!("Failed to create SSH session: {}", e))?;

                session.set_tcp_stream(stream.try_clone()?);

                debug!(target: "ssh", "Performing SSH handshake through SOCKS4 proxy to {}", host.hostname);
                session
                    .handshake()
                    .map_err(|e| anyhow!("SSH handshake through SOCKS4 proxy failed: {}", e))?;

                crate::ssh::auth::Authenticator::authenticate_any(
                    &session, &host, password, passphrase,
                )?;

                if !session.authenticated() {
                    error!(target: "ssh", "Authentication through SOCKS4 proxy failed for {}@{}", host.username, host.hostname);
                    return Err(anyhow!("Authentication through SOCKS4 proxy failed"));
                }

                info!(target: "ssh", "Successfully connected to {}@{} via SOCKS4 proxy", host.username, host.hostname);
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
            ProxyConnection::HttpConnect {
                address,
                port,
                auth,
            } => {
                // Connect to HTTP CONNECT proxy
                let proxy_addr = format!("{}:{}", address, port);
                let target_addr = format!("{}:{}", host.hostname, host.port);
                info!(target: "ssh", "Connecting to {} via HTTP CONNECT proxy {}", target_addr, proxy_addr);

                let socket_addr = proxy_addr
                    .to_socket_addrs()
                    .map_err(|e| anyhow!("Failed to resolve HTTP proxy {}: {}", proxy_addr, e))?
                    .next()
                    .ok_or_else(|| anyhow!("No addresses found for HTTP proxy {}", proxy_addr))?;

                let mut stream = TcpStream::connect_timeout(&socket_addr, CONNECT_TIMEOUT)
                    .map_err(|e| {
                        anyhow!("Connection to HTTP proxy {} failed: {}", proxy_addr, e)
                    })?;

                // HTTP CONNECT request
                debug!(target: "ssh", "Sending HTTP CONNECT request to proxy {}", proxy_addr);
                Self::http_connect_handshake(
                    &mut stream,
                    &host.hostname,
                    host.port,
                    auth.as_ref(),
                )?;
                debug!(target: "ssh", "HTTP CONNECT tunnel established to {}", target_addr);

                stream.set_nonblocking(false)?;

                let mut session =
                    Session::new().map_err(|e| anyhow!("Failed to create SSH session: {}", e))?;

                session.set_tcp_stream(stream.try_clone()?);

                debug!(target: "ssh", "Performing SSH handshake through HTTP proxy to {}", host.hostname);
                session
                    .handshake()
                    .map_err(|e| anyhow!("SSH handshake through HTTP proxy failed: {}", e))?;

                crate::ssh::auth::Authenticator::authenticate_any(
                    &session, &host, password, passphrase,
                )?;

                if !session.authenticated() {
                    error!(target: "ssh", "Authentication through HTTP proxy failed for {}@{}", host.username, host.hostname);
                    return Err(anyhow!("Authentication through HTTP proxy failed"));
                }

                info!(target: "ssh", "Successfully connected to {}@{} via HTTP proxy", host.username, host.hostname);
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
            ProxyConnection::ProxyCommand {
                command,
                target_host,
                target_port,
            } => {
                // Execute proxy command and use stdin/stdout as the socket
                let expanded_command = command
                    .replace("%h", &target_host)
                    .replace("%p", &target_port.to_string());
                info!(target: "ssh", "Connecting to {}:{} via ProxyCommand", target_host, target_port);
                debug!(target: "ssh", "Executing proxy command: {}", expanded_command);

                // Parse command (simple shell-style splitting)
                let parts: Vec<&str> = expanded_command.split_whitespace().collect();
                if parts.is_empty() {
                    return Err(anyhow!("Empty proxy command"));
                }

                let mut child = std::process::Command::new(parts[0])
                    .args(&parts[1..])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .map_err(|e| {
                        error!(target: "ssh", "Failed to execute proxy command '{}': {}", expanded_command, e);
                        anyhow!(
                            "Failed to execute proxy command '{}': {}",
                            expanded_command,
                            e
                        )
                    })?;
                debug!(target: "ssh", "Proxy command started successfully");

                // Create a socket pair to bridge the child process to ssh2's TcpStream requirement
                let (local_stream, remote_stream) = Self::create_socket_pair()?;

                // Get the child's stdin and stdout
                let child_stdin = child
                    .stdin
                    .take()
                    .ok_or_else(|| anyhow!("Failed to get stdin of proxy command"))?;
                let child_stdout = child
                    .stdout
                    .take()
                    .ok_or_else(|| anyhow!("Failed to get stdout of proxy command"))?;

                // Spawn threads to proxy between socket and child process
                std::thread::spawn(move || {
                    Self::proxy_process_to_stream(child_stdin, child_stdout, remote_stream);
                });

                let mut session =
                    Session::new().map_err(|e| anyhow!("Failed to create SSH session: {}", e))?;

                session.set_tcp_stream(local_stream.try_clone()?);

                debug!(target: "ssh", "Performing SSH handshake through proxy command to {}", host.hostname);
                session
                    .handshake()
                    .map_err(|e| anyhow!("SSH handshake through proxy command failed: {}", e))?;

                crate::ssh::auth::Authenticator::authenticate_any(
                    &session, &host, password, passphrase,
                )?;

                if !session.authenticated() {
                    error!(target: "ssh", "Authentication through proxy command failed for {}@{}", host.username, host.hostname);
                    return Err(anyhow!("Authentication through proxy command failed"));
                }

                info!(target: "ssh", "Successfully connected to {}@{} via ProxyCommand", host.username, host.hostname);
                Ok(Self {
                    id: Uuid::new_v4(),
                    host,
                    session,
                    _stream: Some(local_stream),
                    _jump_connection: None,
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

    /// Proxy data between a child process and a stream
    fn proxy_process_to_stream(
        mut child_stdin: std::process::ChildStdin,
        mut child_stdout: std::process::ChildStdout,
        mut stream: TcpStream,
    ) {
        use std::io::{ErrorKind, Read, Write};

        // Set stream to non-blocking
        let _ = stream.set_nonblocking(true);

        let mut buf = [0u8; 8192];

        // We need to handle bidirectional I/O
        // Spawn a thread for each direction
        let stream_clone = stream.try_clone().unwrap();

        // Thread: stream -> child stdin
        let stdin_handle = std::thread::spawn(move || {
            let mut stream = stream_clone;
            let mut buf = [0u8; 8192];
            loop {
                match stream.read(&mut buf) {
                    Ok(0) => break, // Stream closed
                    Ok(n) => {
                        if child_stdin.write_all(&buf[..n]).is_err() {
                            break;
                        }
                        let _ = child_stdin.flush();
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_micros(100));
                    }
                    Err(_) => break,
                }
            }
        });

        // This thread: child stdout -> stream
        loop {
            match child_stdout.read(&mut buf) {
                Ok(0) => break, // Child closed
                Ok(n) => {
                    if stream.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    let _ = stream.flush();
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_micros(100));
                }
                Err(_) => break,
            }
        }

        let _ = stdin_handle.join();
    }

    /// SOCKS5 handshake (RFC 1928)
    fn socks5_handshake(
        stream: &mut TcpStream,
        target_host: &str,
        target_port: u16,
        auth: Option<&(String, String)>,
    ) -> Result<()> {
        use std::io::{Read, Write};

        // Step 1: Send greeting with supported auth methods
        let greeting = if auth.is_some() {
            // Support no-auth (0x00) and username/password (0x02)
            vec![0x05, 0x02, 0x00, 0x02]
        } else {
            // Support only no-auth
            vec![0x05, 0x01, 0x00]
        };
        stream.write_all(&greeting)?;
        stream.flush()?;

        // Step 2: Read server's chosen auth method
        let mut response = [0u8; 2];
        stream.read_exact(&mut response)?;

        if response[0] != 0x05 {
            return Err(anyhow!("Invalid SOCKS5 version in response"));
        }

        match response[1] {
            0x00 => {
                // No authentication required
            }
            0x02 => {
                // Username/password authentication (RFC 1929)
                if let Some((username, password)) = auth {
                    let mut auth_request = vec![0x01]; // Version
                    auth_request.push(username.len() as u8);
                    auth_request.extend(username.as_bytes());
                    auth_request.push(password.len() as u8);
                    auth_request.extend(password.as_bytes());
                    stream.write_all(&auth_request)?;
                    stream.flush()?;

                    let mut auth_response = [0u8; 2];
                    stream.read_exact(&mut auth_response)?;

                    if auth_response[1] != 0x00 {
                        return Err(anyhow!("SOCKS5 authentication failed"));
                    }
                } else {
                    return Err(anyhow!(
                        "SOCKS5 proxy requires authentication but none provided"
                    ));
                }
            }
            0xFF => {
                return Err(anyhow!("SOCKS5 proxy rejected all authentication methods"));
            }
            _ => {
                return Err(anyhow!(
                    "SOCKS5 proxy requires unsupported authentication method: {}",
                    response[1]
                ));
            }
        }

        // Step 3: Send connect request
        let mut request = vec![
            0x05, // Version
            0x01, // Command: CONNECT
            0x00, // Reserved
            0x03, // Address type: domain name
            target_host.len() as u8,
        ];
        request.extend(target_host.as_bytes());
        request.push((target_port >> 8) as u8);
        request.push((target_port & 0xFF) as u8);
        stream.write_all(&request)?;
        stream.flush()?;

        // Step 4: Read connect response
        let mut connect_response = [0u8; 4];
        stream.read_exact(&mut connect_response)?;

        if connect_response[0] != 0x05 {
            return Err(anyhow!("Invalid SOCKS5 version in connect response"));
        }

        if connect_response[1] != 0x00 {
            let error_msg = match connect_response[1] {
                0x01 => "general SOCKS server failure",
                0x02 => "connection not allowed by ruleset",
                0x03 => "network unreachable",
                0x04 => "host unreachable",
                0x05 => "connection refused",
                0x06 => "TTL expired",
                0x07 => "command not supported",
                0x08 => "address type not supported",
                _ => "unknown error",
            };
            return Err(anyhow!("SOCKS5 connect failed: {}", error_msg));
        }

        // Read and discard the bound address
        match connect_response[3] {
            0x01 => {
                // IPv4: 4 bytes + 2 port bytes
                let mut discard = [0u8; 6];
                stream.read_exact(&mut discard)?;
            }
            0x03 => {
                // Domain: 1 length byte + domain + 2 port bytes
                let mut len = [0u8; 1];
                stream.read_exact(&mut len)?;
                let mut discard = vec![0u8; len[0] as usize + 2];
                stream.read_exact(&mut discard)?;
            }
            0x04 => {
                // IPv6: 16 bytes + 2 port bytes
                let mut discard = [0u8; 18];
                stream.read_exact(&mut discard)?;
            }
            _ => {
                return Err(anyhow!("SOCKS5 unsupported address type in response"));
            }
        }

        Ok(())
    }

    /// SOCKS4 handshake (SOCKS4a with domain name support)
    fn socks4_handshake(
        stream: &mut TcpStream,
        target_host: &str,
        target_port: u16,
        user_id: Option<&str>,
    ) -> Result<()> {
        use std::io::{Read, Write};

        // SOCKS4a request format:
        // 1 byte: version (0x04)
        // 1 byte: command (0x01 = CONNECT)
        // 2 bytes: port (big-endian)
        // 4 bytes: IP address (0.0.0.x for SOCKS4a to indicate domain follows)
        // variable: user ID (null-terminated)
        // variable: domain name (null-terminated, only for SOCKS4a)

        let mut request = vec![
            0x04, // Version
            0x01, // Command: CONNECT
            (target_port >> 8) as u8,
            (target_port & 0xFF) as u8,
            0x00,
            0x00,
            0x00,
            0x01, // Invalid IP for SOCKS4a (domain follows)
        ];

        // Add user ID (null-terminated)
        if let Some(uid) = user_id {
            request.extend(uid.as_bytes());
        }
        request.push(0x00); // Null terminator for user ID

        // Add domain name (null-terminated) for SOCKS4a
        request.extend(target_host.as_bytes());
        request.push(0x00); // Null terminator for domain

        stream.write_all(&request)?;
        stream.flush()?;

        // Read response (8 bytes)
        let mut response = [0u8; 8];
        stream.read_exact(&mut response)?;

        // Check response code (byte 1)
        match response[1] {
            0x5A => Ok(()), // Request granted
            0x5B => Err(anyhow!("SOCKS4 request rejected or failed")),
            0x5C => Err(anyhow!("SOCKS4 request failed: client not running identd")),
            0x5D => Err(anyhow!(
                "SOCKS4 request failed: identd could not confirm user"
            )),
            _ => Err(anyhow!("SOCKS4 unknown response code: {}", response[1])),
        }
    }

    /// HTTP CONNECT handshake
    fn http_connect_handshake(
        stream: &mut TcpStream,
        target_host: &str,
        target_port: u16,
        auth: Option<&(String, String)>,
    ) -> Result<()> {
        use std::io::{BufRead, BufReader, Write};

        // Build CONNECT request
        let mut request = format!(
            "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n",
            target_host, target_port, target_host, target_port
        );

        // Add Proxy-Authorization header if credentials provided
        if let Some((username, password)) = auth {
            let credentials = format!("{}:{}", username, password);
            let encoded = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                credentials.as_bytes(),
            );
            request.push_str(&format!("Proxy-Authorization: Basic {}\r\n", encoded));
        }

        request.push_str("\r\n");

        stream.write_all(request.as_bytes())?;
        stream.flush()?;

        // Read response
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut status_line = String::new();
        reader.read_line(&mut status_line)?;

        // Parse status line (e.g., "HTTP/1.1 200 Connection established")
        let parts: Vec<&str> = status_line.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(anyhow!("Invalid HTTP response from proxy"));
        }

        let status_code: u16 = parts[1]
            .parse()
            .map_err(|_| anyhow!("Invalid HTTP status code from proxy"))?;

        if status_code != 200 {
            return Err(anyhow!(
                "HTTP proxy returned status {}: {}",
                status_code,
                parts.get(2..).map(|p| p.join(" ")).unwrap_or_default()
            ));
        }

        // Read and discard headers until empty line
        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            if line.trim().is_empty() {
                break;
            }
        }

        Ok(())
    }

    /// Get a reference to the SSH session
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Open an interactive shell channel
    pub fn open_shell(&mut self, cols: u32, rows: u32) -> Result<ssh2::Channel> {
        debug!(target: "ssh", "Opening shell channel for {}@{} ({}x{})", self.host.username, self.host.hostname, cols, rows);
        let mut channel = self
            .session
            .channel_session()
            .map_err(|e| anyhow!("Failed to open channel: {}", e))?;

        // Request PTY
        channel
            .request_pty("xterm-256color", None, Some((cols, rows, 0, 0)))
            .map_err(|e| anyhow!("Failed to request PTY: {}", e))?;

        // Request shell
        channel
            .shell()
            .map_err(|e| anyhow!("Failed to request shell: {}", e))?;

        // Set session to non-blocking for async I/O
        self.session.set_blocking(false);

        // Also set the underlying stream to non-blocking
        // This is critical because ssh2 doesn't automatically set the stream to non-blocking
        // and if the stream remains blocking, session.read() will block despite session.set_blocking(false)
        if let Some(stream) = self._stream.as_ref() {
            stream
                .set_nonblocking(true)
                .map_err(|e| anyhow!("Failed to set stream non-blocking: {}", e))?;
        }

        info!(target: "ssh", "Shell channel opened for {}@{}", self.host.username, self.host.hostname);
        Ok(channel)
    }

    /// Open a direct TCP/IP channel (for port forwarding)
    pub fn open_direct_tcpip(&self, host: &str, port: u16) -> Result<ssh2::Channel> {
        debug!(target: "ssh", "Opening direct-tcpip channel to {}:{}", host, port);
        self.session
            .channel_direct_tcpip(host, port, None)
            .map_err(|e| anyhow!("Failed to open direct-tcpip channel: {}", e))
    }

    /// Open SFTP subsystem
    /// Note: The session stays in blocking mode for SFTP operations
    /// SFTP connections should be dedicated (separate from shell connections)
    pub fn open_sftp(&self) -> Result<ssh2::Sftp> {
        debug!(target: "ssh", "Opening SFTP subsystem for {}@{}", self.host.username, self.host.hostname);
        // SFTP operations require blocking mode
        // Since SFTP sessions should use dedicated connections (not shared with shell),
        // we keep the session in blocking mode for all SFTP operations to work correctly
        self.session.set_blocking(true);

        let sftp = self
            .session
            .sftp()
            .map_err(|e| anyhow!("Failed to open SFTP: {}", e))?;

        // Keep in blocking mode - SFTP operations (readdir, create, read, write)
        // all require blocking mode to work correctly

        info!(target: "ssh", "SFTP subsystem opened for {}@{}", self.host.username, self.host.hostname);
        Ok(sftp)
    }

    /// Check if connection is alive
    pub fn is_alive(&self) -> bool {
        self.session.authenticated() && self.status == ConnectionStatus::Authenticated
    }

    /// Execute a command and return output
    pub fn exec(&self, command: &str) -> Result<String> {
        let mut channel = self
            .session
            .channel_session()
            .map_err(|e| anyhow!("Failed to open channel: {}", e))?;

        channel
            .exec(command)
            .map_err(|e| anyhow!("Failed to execute command: {}", e))?;

        let mut output = String::new();
        channel.read_to_string(&mut output)?;

        channel.wait_close()?;

        Ok(output)
    }

    /// Close the connection
    pub fn close(self) -> Result<()> {
        self.session
            .disconnect(None, "Goodbye", None)
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
