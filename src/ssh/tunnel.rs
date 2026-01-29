//! SSH tunneling (port forwarding)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
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
