//! SSH tunneling (port forwarding)

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::Receiver;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Tunnel type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TunnelType {
    /// Local port forwarding (-L)
    /// Forwards connections from local port to remote host
    Local {
        bind_addr: SocketAddr,
        remote_host: String,
        remote_port: u16,
    },
    /// Remote port forwarding (-R)
    /// Forwards connections from remote port to local host
    Remote {
        remote_addr: SocketAddr,
        local_host: String,
        local_port: u16,
    },
    /// Dynamic SOCKS proxy (-D)
    Dynamic { bind_addr: SocketAddr },
}

/// Tunnel statistics
#[derive(Debug, Clone, Default)]
pub struct TunnelStats {
    /// Bytes sent through tunnel
    pub bytes_sent: u64,
    /// Bytes received through tunnel
    pub bytes_received: u64,
    /// Active connections
    pub active_connections: u32,
    /// Total connections
    pub total_connections: u64,
}

/// Active tunnel
pub struct Tunnel {
    /// Tunnel ID
    pub id: Uuid,
    /// Tunnel name
    pub name: String,
    /// Tunnel type and configuration
    pub tunnel_type: TunnelType,
    /// Whether tunnel is active
    pub active: bool,
    /// Statistics
    pub stats: TunnelStats,
    /// Shutdown sender
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl Tunnel {
    /// Create a new tunnel (not yet started)
    pub fn new(name: String, tunnel_type: TunnelType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            tunnel_type,
            active: false,
            stats: TunnelStats::default(),
            shutdown_tx: None,
        }
    }

    /// Get description of the tunnel
    pub fn description(&self) -> String {
        match &self.tunnel_type {
            TunnelType::Local {
                bind_addr,
                remote_host,
                remote_port,
            } => {
                format!("L:{} → {}:{}", bind_addr, remote_host, remote_port)
            }
            TunnelType::Remote {
                remote_addr,
                local_host,
                local_port,
            } => {
                format!("R:{} → {}:{}", remote_addr, local_host, local_port)
            }
            TunnelType::Dynamic { bind_addr } => {
                format!("D:{}", bind_addr)
            }
        }
    }

    /// Stop the tunnel
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
        self.active = false;
        Ok(())
    }
}

/// Tunnel manager
pub struct TunnelManager {
    tunnels: HashMap<Uuid, Tunnel>,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self {
            tunnels: HashMap::new(),
        }
    }

    /// Add a tunnel (not started)
    pub fn add(&mut self, tunnel: Tunnel) -> Uuid {
        let id = tunnel.id;
        self.tunnels.insert(id, tunnel);
        id
    }

    /// Get a tunnel by ID
    pub fn get(&self, id: Uuid) -> Option<&Tunnel> {
        self.tunnels.get(&id)
    }

    /// Get a mutable tunnel
    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut Tunnel> {
        self.tunnels.get_mut(&id)
    }

    /// Remove a tunnel
    pub async fn remove(&mut self, id: Uuid) -> Result<Option<Tunnel>> {
        if let Some(mut tunnel) = self.tunnels.remove(&id) {
            tunnel.stop().await?;
            Ok(Some(tunnel))
        } else {
            Ok(None)
        }
    }

    /// List all tunnels
    pub fn list(&self) -> Vec<&Tunnel> {
        self.tunnels.values().collect()
    }

    /// Stop all tunnels
    pub async fn stop_all(&mut self) -> Result<()> {
        for tunnel in self.tunnels.values_mut() {
            tunnel.stop().await?;
        }
        Ok(())
    }
}

impl Default for TunnelManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Run a tunnel in the current thread (blocking).
pub fn run_tunnel<F>(
    mut connection: crate::ssh::SshConnection,
    tunnel: crate::config::TunnelConfig,
    shutdown: Receiver<()>,
    report: &mut F,
) -> Result<()>
where
    F: FnMut(&str),
{
    match tunnel {
        crate::config::TunnelConfig::Local {
            bind_addr,
            bind_port,
            remote_host,
            remote_port,
            name,
            ..
        } => run_local_forward(
            &mut connection,
            bind_addr,
            bind_port,
            remote_host,
            remote_port,
            name,
            shutdown,
            report,
        ),
        crate::config::TunnelConfig::Remote {
            remote_addr,
            remote_port,
            local_host,
            local_port,
            name,
            ..
        } => run_remote_forward(
            &mut connection,
            remote_addr,
            remote_port,
            local_host,
            local_port,
            name,
            shutdown,
            report,
        ),
        crate::config::TunnelConfig::Dynamic {
            bind_addr,
            bind_port,
            name,
            ..
        } => run_dynamic_forward(
            &mut connection,
            bind_addr,
            bind_port,
            name,
            shutdown,
            report,
        ),
    }
}

fn run_local_forward<F>(
    connection: &mut crate::ssh::SshConnection,
    bind_addr: String,
    bind_port: u16,
    remote_host: String,
    remote_port: u16,
    name: String,
    shutdown: Receiver<()>,
    report: &mut F,
) -> Result<()>
where
    F: FnMut(&str),
{
    let listener = TcpListener::bind((bind_addr.as_str(), bind_port)).map_err(|e| {
        anyhow!(
            "Failed to bind local tunnel {}:{}: {}",
            bind_addr,
            bind_port,
            e
        )
    })?;
    listener.set_nonblocking(true)?;

    report(&format!("Tunnel started: {}", name));

    tracing::info!(
        target: "ssh::tunnel",
        "Local tunnel listening on {}:{} -> {}:{}",
        bind_addr,
        bind_port,
        remote_host,
        remote_port
    );

    let session = connection.session_ref().clone();
    loop {
        if should_shutdown(&shutdown) {
            break;
        }

        match listener.accept() {
            Ok((stream, _)) => {
                let session = session.clone();
                let remote_host = remote_host.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_local_stream(session, stream, remote_host, remote_port) {
                        tracing::warn!(target: "ssh::tunnel", "Local tunnel error: {}", e);
                    }
                });
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::WouldBlock {
                    tracing::warn!(target: "ssh::tunnel", "Local tunnel accept failed: {}", e);
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        }
    }

    Ok(())
}

fn handle_local_stream(
    session: ssh2::Session,
    stream: TcpStream,
    remote_host: String,
    remote_port: u16,
) -> Result<()> {
    let channel = session
        .channel_direct_tcpip(&remote_host, remote_port, None)
        .map_err(|e| anyhow!("Failed to open direct-tcpip channel: {}", e))?;

    proxy_tcp_channel(stream, channel);
    Ok(())
}

fn run_remote_forward<F>(
    connection: &mut crate::ssh::SshConnection,
    remote_addr: String,
    remote_port: u16,
    local_host: String,
    local_port: u16,
    name: String,
    shutdown: Receiver<()>,
    report: &mut F,
) -> Result<()>
where
    F: FnMut(&str),
{
    let session = connection.session_ref().clone();
    let (mut listener, bound_port) = session
        .channel_forward_listen(remote_port, Some(remote_addr.as_str()), None)
        .map_err(|e| anyhow!("Failed to set up remote forward: {}", e))?;
    session.set_blocking(false);

    report(&format!("Tunnel started: {}", name));

    tracing::info!(
        target: "ssh::tunnel",
        "Remote tunnel listening on {}:{} -> {}:{}",
        remote_addr,
        bound_port,
        local_host,
        local_port
    );

    loop {
        if should_shutdown(&shutdown) {
            break;
        }
        match listener.accept() {
            Ok(channel) => {
                let local_host = local_host.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_remote_channel(channel, local_host, local_port) {
                        tracing::warn!(target: "ssh::tunnel", "Remote tunnel error: {}", e);
                    }
                });
            }
            Err(e) => {
                let msg = e.to_string();
                let kind = std::io::Error::from(e).kind();
                if kind != std::io::ErrorKind::WouldBlock {
                    tracing::warn!(target: "ssh::tunnel", "Remote tunnel accept failed: {}", msg);
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        }
    }

    Ok(())
}

fn handle_remote_channel(
    channel: ssh2::Channel,
    local_host: String,
    local_port: u16,
) -> Result<()> {
    let stream = TcpStream::connect((local_host.as_str(), local_port))
        .map_err(|e| anyhow!("Failed to connect to local target: {}", e))?;

    proxy_tcp_channel(stream, channel);
    Ok(())
}

fn run_dynamic_forward<F>(
    connection: &mut crate::ssh::SshConnection,
    bind_addr: String,
    bind_port: u16,
    name: String,
    shutdown: Receiver<()>,
    report: &mut F,
) -> Result<()>
where
    F: FnMut(&str),
{
    let listener = TcpListener::bind((bind_addr.as_str(), bind_port)).map_err(|e| {
        anyhow!(
            "Failed to bind dynamic tunnel {}:{}: {}",
            bind_addr,
            bind_port,
            e
        )
    })?;
    listener.set_nonblocking(true)?;

    report(&format!("Tunnel started: {}", name));

    tracing::info!(
        target: "ssh::tunnel",
        "Dynamic SOCKS5 tunnel listening on {}:{}",
        bind_addr,
        bind_port
    );

    let session = connection.session_ref().clone();
    loop {
        if should_shutdown(&shutdown) {
            break;
        }

        match listener.accept() {
            Ok((stream, _)) => {
                let session = session.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_socks5_stream(session, stream) {
                        tracing::warn!(target: "ssh::tunnel", "SOCKS5 error: {}", e);
                    }
                });
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::WouldBlock {
                    tracing::warn!(target: "ssh::tunnel", "SOCKS5 accept failed: {}", e);
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        }
    }

    Ok(())
}

fn handle_socks5_stream(session: ssh2::Session, mut stream: TcpStream) -> Result<()> {
    stream.set_nodelay(true).ok();

    let mut header = [0u8; 2];
    stream.read_exact(&mut header)?;
    if header[0] != 0x05 {
        return Err(anyhow!("Invalid SOCKS version"));
    }

    let nmethods = header[1] as usize;
    let mut methods = vec![0u8; nmethods];
    stream.read_exact(&mut methods)?;

    // No authentication
    stream.write_all(&[0x05, 0x00])?;

    let mut req = [0u8; 4];
    stream.read_exact(&mut req)?;
    if req[0] != 0x05 {
        return Err(anyhow!("Invalid SOCKS request version"));
    }
    if req[1] != 0x01 {
        send_socks5_reply(&mut stream, 0x07)?;
        return Err(anyhow!("Unsupported SOCKS command"));
    }

    let addr = match req[3] {
        0x01 => {
            let mut ip = [0u8; 4];
            stream.read_exact(&mut ip)?;
            std::net::Ipv4Addr::from(ip).to_string()
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len)?;
            let mut domain = vec![0u8; len[0] as usize];
            stream.read_exact(&mut domain)?;
            String::from_utf8(domain)?
        }
        0x04 => {
            let mut ip = [0u8; 16];
            stream.read_exact(&mut ip)?;
            std::net::Ipv6Addr::from(ip).to_string()
        }
        _ => {
            send_socks5_reply(&mut stream, 0x08)?;
            return Err(anyhow!("Unsupported SOCKS address type"));
        }
    };

    let mut port_buf = [0u8; 2];
    stream.read_exact(&mut port_buf)?;
    let port = u16::from_be_bytes(port_buf);

    let channel = match session.channel_direct_tcpip(&addr, port, None) {
        Ok(channel) => channel,
        Err(e) => {
            send_socks5_reply(&mut stream, 0x05)?;
            return Err(anyhow!("Failed to open SOCKS channel: {}", e));
        }
    };

    send_socks5_reply(&mut stream, 0x00)?;
    proxy_tcp_channel(stream, channel);
    Ok(())
}

fn send_socks5_reply(stream: &mut TcpStream, status: u8) -> Result<()> {
    // Bind addr/port set to 0.0.0.0:0
    let reply = [0x05, status, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
    stream.write_all(&reply)?;
    Ok(())
}

fn should_shutdown(shutdown: &Receiver<()>) -> bool {
    match shutdown.try_recv() {
        Ok(()) => true,
        Err(std::sync::mpsc::TryRecvError::Empty) => false,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
    }
}

fn proxy_tcp_channel(stream: TcpStream, channel: ssh2::Channel) {
    let mut local_read = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(target: "ssh::tunnel", "Clone stream failed: {}", e);
            return;
        }
    };
    let mut local_write = stream;
    let mut chan_read = channel.stream(0);
    let mut chan_write = channel.stream(0);

    std::thread::spawn(move || {
        let _ = std::io::copy(&mut local_read, &mut chan_write);
        let _ = chan_write.flush();
    });

    std::thread::spawn(move || {
        let _ = std::io::copy(&mut chan_read, &mut local_write);
        let _ = local_write.shutdown(Shutdown::Both);
    });
}
