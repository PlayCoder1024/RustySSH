//! Configuration management

mod hosts;
mod settings;

pub use hosts::{AuthMethod, HostConfig, HostGroup, JumpHostRef};
pub use settings::Settings;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Application settings
    #[serde(default)]
    pub settings: Settings,
    /// Host groups
    #[serde(default)]
    pub groups: Vec<HostGroup>,
    /// Ungrouped hosts
    #[serde(default)]
    pub hosts: Vec<HostConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            groups: vec![
                HostGroup {
                    name: "Production".to_string(),
                    hosts: vec![],
                    expanded: true,
                },
                HostGroup {
                    name: "Development".to_string(),
                    hosts: vec![],
                    expanded: true,
                },
            ],
            hosts: vec![],
        }
    }
}

impl Config {
    /// Get config file path
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustyssh")
            .join("config.yaml")
    }

    /// Load configuration from file
    pub async fn load() -> Result<Self> {
        let path = Self::config_path();

        if path.exists() {
            let content = tokio::fs::read_to_string(&path).await?;
            let config: Config = serde_yaml::from_str(&content)?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save().await?;
            Ok(config)
        }
    }

    /// Save configuration to file
    pub async fn save(&self) -> Result<()> {
        let path = Self::config_path();

        // Create parent directories
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let content = serde_yaml::to_string(self)?;
        tokio::fs::write(&path, content).await?;
        Ok(())
    }

    /// Get all hosts (grouped and ungrouped)
    pub fn all_hosts(&self) -> Vec<&HostConfig> {
        let mut hosts: Vec<&HostConfig> = self.hosts.iter().collect();
        for group in &self.groups {
            hosts.extend(group.hosts.iter());
        }
        hosts
    }

    /// Find host by ID
    pub fn find_host(&self, id: uuid::Uuid) -> Option<&HostConfig> {
        self.all_hosts().into_iter().find(|h| h.id == id)
    }

    /// Resolve a jump host reference to a HostConfig
    /// Tries to match by UUID first, then by hostname, then by name
    pub fn resolve_jump_host(&self, reference: &JumpHostRef) -> Option<&HostConfig> {
        let hosts = self.all_hosts();
        match reference {
            JumpHostRef::ByUuid(uuid) => hosts.into_iter().find(|h| &h.id == uuid),
            JumpHostRef::ByHostname(hostname) => {
                // First try exact hostname match
                if let Some(host) = hosts.iter().find(|h| &h.hostname == hostname) {
                    return Some(host);
                }
                // Then try by name (connection name)
                hosts.into_iter().find(|h| &h.name == hostname)
            }
            JumpHostRef::ByName(name) => {
                // First try by name
                if let Some(host) = hosts.iter().find(|h| &h.name == name) {
                    return Some(host);
                }
                // Then try by hostname
                hosts.into_iter().find(|h| &h.hostname == name)
            }
        }
    }

    /// Add a new host
    pub fn add_host(&mut self, host: HostConfig, group_name: Option<&str>) {
        if let Some(name) = group_name {
            if let Some(group) = self.groups.iter_mut().find(|g| g.name == name) {
                group.hosts.push(host);
                return;
            }
        }
        self.hosts.push(host);
    }
}
