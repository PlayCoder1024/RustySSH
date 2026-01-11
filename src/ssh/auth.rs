//! SSH authentication handling

use crate::config::{AuthMethod, HostConfig};
use anyhow::{anyhow, Result};
use ssh2::Session;
use std::path::Path;

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
                let password = password.ok_or_else(|| anyhow!("Password required"))?;
                session
                    .userauth_password(&host.username, password)
                    .map_err(|e| anyhow!("Password auth failed: {}", e))
            }
            AuthMethod::KeyFile { path, passphrase_required } => {
                let passphrase = if *passphrase_required { passphrase } else { None };
                Self::auth_with_key(session, &host.username, path, passphrase)
            }
            AuthMethod::Agent => {
                Self::auth_with_agent(session, &host.username)
            }
            AuthMethod::Certificate { cert_path: _, key_path } => {
                // For now, treat as key file auth
                Self::auth_with_key(session, &host.username, key_path, passphrase)
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
        let pub_key_path = key_path.with_extension("pub");
        let pub_key = if pub_key_path.exists() {
            Some(pub_key_path.as_path())
        } else {
            None
        };
        
        session
            .userauth_pubkey_file(username, pub_key, key_path, passphrase)
            .map_err(|e| anyhow!("Key auth failed: {}", e))
    }

    /// Authenticate with SSH agent
    fn auth_with_agent(
        session: &Session,
        username: &str,
    ) -> Result<()> {
        let mut agent = session.agent()
            .map_err(|e| anyhow!("Failed to connect to SSH agent: {}", e))?;
        
        agent.connect()
            .map_err(|e| anyhow!("Failed to connect to agent: {}", e))?;
        
        agent.list_identities()
            .map_err(|e| anyhow!("Failed to list agent identities: {}", e))?;
        
        // Try each identity
        for identity in agent.identities()? {
            if agent.userauth(username, &identity).is_ok() {
                return Ok(());
            }
        }
        
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
        let methods = session.auth_methods(&host.username)
            .unwrap_or_default();
        
        tracing::debug!("Available auth methods: {}", methods);

        // Try agent first (most convenient)
        if methods.contains("publickey") {
            if let Ok(()) = Self::auth_with_agent(session, &host.username) {
                return Ok(());
            }
        }

        // Then try configured method
        Self::authenticate(session, host, password, passphrase)
    }
}
