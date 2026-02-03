//! SSH authentication handling

use crate::config::{AuthMethod, HostConfig};
use crate::utils::resolve_ssh_key_path;
use anyhow::{anyhow, Result};
use ssh2::Session;
use std::path::Path;
use tracing::{debug, info, warn};

/// Authentication handler
pub struct Authenticator;

impl Authenticator {
    /// Authenticate with the given method
    pub fn authenticate(
        session: &Session,
        host: &HostConfig,
        password: Option<&str>,
        passphrase: Option<&str>,
    ) -> Result<()> {
        match &host.auth {
            AuthMethod::Password => {
                debug!(target: "ssh::auth", "Attempting password authentication for {}@{}", host.username, host.hostname);
                let password = password.ok_or_else(|| anyhow!("Password required"))?;
                session
                    .userauth_password(&host.username, password)
                    .map_err(|e| {
                        warn!(target: "ssh::auth", "Password authentication failed for {}@{}: {}", host.username, host.hostname, e);
                        anyhow!("Password auth failed: {}", e)
                    })?;
                info!(target: "ssh::auth", "Password authentication successful for {}@{}", host.username, host.hostname);
                Ok(())
            }
            AuthMethod::KeyFile {
                path,
                passphrase_required,
            } => {
                let resolved_path = resolve_ssh_key_path(path);
                debug!(target: "ssh::auth", "Attempting key authentication for {}@{} with key {}", host.username, host.hostname, resolved_path.display());
                if !resolved_path.exists() {
                    warn!(target: "ssh::auth", "SSH key not found: {} (resolved to {})", path.display(), resolved_path.display());
                    return Err(anyhow!(
                        "SSH key not found: {} (resolved to {})",
                        path.display(),
                        resolved_path.display()
                    ));
                }
                let passphrase = if *passphrase_required {
                    passphrase
                } else {
                    None
                };
                Self::auth_with_key(session, &host.username, &resolved_path, passphrase)
            }
            AuthMethod::Agent => {
                debug!(target: "ssh::auth", "Attempting agent authentication for {}@{}", host.username, host.hostname);
                Self::auth_with_agent(session, &host.username)
            }
            AuthMethod::Certificate {
                cert_path: _,
                key_path,
            } => {
                let resolved_path = resolve_ssh_key_path(key_path);
                debug!(target: "ssh::auth", "Attempting certificate authentication for {}@{} with key {}", host.username, host.hostname, resolved_path.display());
                if !resolved_path.exists() {
                    warn!(target: "ssh::auth", "SSH key not found: {} (resolved to {})", key_path.display(), resolved_path.display());
                    return Err(anyhow!(
                        "SSH key not found: {} (resolved to {})",
                        key_path.display(),
                        resolved_path.display()
                    ));
                }
                // For now, treat as key file auth
                Self::auth_with_key(session, &host.username, &resolved_path, passphrase)
            }
        }
    }

    /// Authenticate with private key file
    fn auth_with_key(
        session: &Session,
        username: &str,
        key_path: &Path,
        passphrase: Option<&str>,
    ) -> Result<()> {
        debug!(target: "ssh::auth", "Using key file: {}", key_path.display());
        let pub_key_path = key_path.with_extension("pub");
        let pub_key = if pub_key_path.exists() {
            debug!(target: "ssh::auth", "Found public key: {}", pub_key_path.display());
            Some(pub_key_path.as_path())
        } else {
            debug!(target: "ssh::auth", "No public key found at {}", pub_key_path.display());
            None
        };

        session
            .userauth_pubkey_file(username, pub_key, key_path, passphrase)
            .map_err(|e| {
                warn!(target: "ssh::auth", "Key authentication failed for {}: {}", username, e);
                anyhow!("Key auth failed: {}", e)
            })?;
        info!(target: "ssh::auth", "Key authentication successful for {}", username);
        Ok(())
    }

    /// Authenticate with SSH agent
    fn auth_with_agent(session: &Session, username: &str) -> Result<()> {
        debug!(target: "ssh::auth", "Connecting to SSH agent");
        let mut agent = session
            .agent()
            .map_err(|e| anyhow!("Failed to connect to SSH agent: {}", e))?;

        agent
            .connect()
            .map_err(|e| anyhow!("Failed to connect to agent: {}", e))?;
        debug!(target: "ssh::auth", "Connected to SSH agent");

        agent
            .list_identities()
            .map_err(|e| anyhow!("Failed to list agent identities: {}", e))?;

        let identities: Vec<_> = agent.identities()?.into_iter().collect();
        debug!(target: "ssh::auth", "SSH agent has {} identities", identities.len());

        // Try each identity
        for identity in identities {
            if agent.userauth(username, &identity).is_ok() {
                info!(target: "ssh::auth", "Agent authentication successful for {}", username);
                return Ok(());
            }
        }

        warn!(target: "ssh::auth", "No agent identity authenticated successfully for {}", username);
        Err(anyhow!("No agent identity authenticated successfully"))
    }

    /// Try all authentication methods in order
    pub fn authenticate_any(
        session: &Session,
        host: &HostConfig,
        password: Option<&str>,
        passphrase: Option<&str>,
    ) -> Result<()> {
        // Get available auth methods
        let methods = session.auth_methods(&host.username).unwrap_or_default();

        debug!(target: "ssh::auth", "Available auth methods for {}@{}: {}", host.username, host.hostname, methods);

        // Try agent first (most convenient)
        if methods.contains("publickey") {
            debug!(target: "ssh::auth", "Trying agent authentication first for {}@{}", host.username, host.hostname);
            if let Ok(()) = Self::auth_with_agent(session, &host.username) {
                return Ok(());
            }
            debug!(target: "ssh::auth", "Agent authentication failed, falling back to configured method");
        }

        // Then try configured method
        debug!(target: "ssh::auth", "Using configured auth method: {:?}", host.auth);
        Self::authenticate(session, host, password, passphrase)
    }
}
