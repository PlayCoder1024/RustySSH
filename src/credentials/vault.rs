//! Encrypted credential vault
//!
//! Stores encrypted passwords using AES-256-GCM

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info, warn};
use uuid::Uuid;
use zeroize::Zeroize;

/// A stored credential (encrypted password with nonce)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredential {
    /// Encrypted password (base64 encoded)
    pub ciphertext: String,
    /// Nonce used for encryption (base64 encoded)
    pub nonce: String,
}

/// Encrypted credential vault
#[derive(Debug, Serialize, Deserialize)]
pub struct CredentialVault {
    /// Version for future format migrations
    pub version: u32,
    /// Stored credentials by host ID
    #[serde(default)]
    pub credentials: HashMap<Uuid, StoredCredential>,
}

impl CredentialVault {
    /// Get the vault file path
    pub fn vault_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustyssh")
            .join("credentials.enc")
    }

    /// Load the vault from disk
    pub async fn load() -> Result<Self> {
        let path = Self::vault_path();
        debug!(target: "credentials", "Loading credential vault from {}", path.display());

        if path.exists() {
            let content = tokio::fs::read_to_string(&path).await?;
            let vault: CredentialVault = serde_json::from_str(&content)?;
            info!(target: "credentials", "Loaded credential vault with {} credentials", vault.credentials.len());
            Ok(vault)
        } else {
            debug!(target: "credentials", "No existing vault found, creating new one");
            Ok(Self::default())
        }
    }

    /// Save the vault to disk
    pub async fn save(&self) -> Result<()> {
        let path = Self::vault_path();
        debug!(target: "credentials", "Saving credential vault to {}", path.display());

        // Create parent directories
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&path, content).await?;
        info!(target: "credentials", "Saved credential vault with {} credentials", self.credentials.len());
        Ok(())
    }

    /// Store a password (encrypted)
    pub fn store(&mut self, host_id: Uuid, password: &str, key: &[u8; 32]) -> Result<()> {
        debug!(target: "credentials", "Storing credential for host {}", host_id);
        // Generate random nonce (96 bits for GCM)
        let mut nonce_bytes = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);

        // Create cipher
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| anyhow!("Failed to create cipher: {}", e))?;

        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, password.as_bytes())
            .map_err(|e| anyhow!("Failed to encrypt password: {}", e))?;

        // Store as base64
        use base64::Engine;
        let credential = StoredCredential {
            ciphertext: base64::engine::general_purpose::STANDARD.encode(&ciphertext),
            nonce: base64::engine::general_purpose::STANDARD.encode(&nonce_bytes),
        };

        self.credentials.insert(host_id, credential);
        info!(target: "credentials", "Credential stored for host {}", host_id);
        Ok(())
    }

    /// Retrieve a password (decrypted)
    pub fn retrieve(&self, host_id: Uuid, key: &[u8; 32]) -> Result<Option<String>> {
        debug!(target: "credentials", "Retrieving credential for host {}", host_id);
        let credential = match self.credentials.get(&host_id) {
            Some(c) => c,
            None => {
                debug!(target: "credentials", "No credential found for host {}", host_id);
                return Ok(None);
            }
        };

        use base64::Engine;

        // Decode from base64
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(&credential.ciphertext)
            .map_err(|e| anyhow!("Failed to decode ciphertext: {}", e))?;

        let nonce_bytes = base64::engine::general_purpose::STANDARD
            .decode(&credential.nonce)
            .map_err(|e| anyhow!("Failed to decode nonce: {}", e))?;

        if nonce_bytes.len() != 12 {
            return Err(anyhow!("Invalid nonce length"));
        }

        // Create cipher
        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| anyhow!("Failed to create cipher: {}", e))?;

        let nonce = Nonce::from_slice(&nonce_bytes);

        // Decrypt
        let mut plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| {
                warn!(target: "credentials", "Failed to decrypt credential for host {}: {}", host_id, e);
                anyhow!("Failed to decrypt password: {}", e)
            })?;

        let password = String::from_utf8(plaintext.clone())
            .map_err(|e| anyhow!("Invalid UTF-8 in decrypted password: {}", e))?;

        // Zeroize the plaintext buffer
        plaintext.zeroize();

        debug!(target: "credentials", "Credential retrieved for host {}", host_id);
        Ok(Some(password))
    }

    /// Check if a credential exists for a host
    pub fn has_credential(&self, host_id: Uuid) -> bool {
        self.credentials.contains_key(&host_id)
    }

    /// Remove a credential
    pub fn remove(&mut self, host_id: Uuid) {
        self.credentials.remove(&host_id);
    }

    /// List all hosts with stored credentials
    pub fn list_hosts(&self) -> Vec<Uuid> {
        self.credentials.keys().copied().collect()
    }
}

impl Default for CredentialVault {
    fn default() -> Self {
        Self {
            version: 1,
            credentials: HashMap::new(),
        }
    }
}
