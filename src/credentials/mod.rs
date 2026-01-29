//! Credential management for secure password storage
//!
//! This module provides secure storage for SSH passwords using:
//! - Argon2id for master password hashing and key derivation
//! - AES-256-GCM for password encryption
//! - OS keyring for master password hash storage

mod master;
mod vault;

pub use master::MasterPassword;
pub use vault::{CredentialVault, StoredCredential};

use anyhow::Result;
use uuid::Uuid;

/// Service name for keyring storage
pub const KEYRING_SERVICE: &str = "rustyssh";

/// Credential manager - main interface for password operations
pub struct CredentialManager {
    /// Master password handler
    master: MasterPassword,
    /// Encrypted credential vault
    vault: CredentialVault,
    /// Derived encryption key (only present when unlocked)
    encryption_key: Option<[u8; 32]>,
    /// Whether master password is unlocked this session
    unlocked: bool,
}

impl CredentialManager {
    /// Create a new credential manager
    pub async fn new() -> Result<Self> {
        let master = MasterPassword::new();
        let vault = CredentialVault::load().await?;

        Ok(Self {
            master,
            vault,
            encryption_key: None,
            unlocked: false,
        })
    }

    /// Check if a master password has been set
    pub fn has_master_password(&self) -> bool {
        self.master.is_set()
    }

    /// Check if currently unlocked
    pub fn is_unlocked(&self) -> bool {
        self.unlocked
    }

    /// Set up a new master password (first time setup)
    pub fn setup_master_password(&mut self, password: &str) -> Result<()> {
        self.master.setup(password)?;
        let key = self.master.derive_encryption_key(password)?;
        self.encryption_key = Some(key);
        self.unlocked = true;
        Ok(())
    }

    /// Unlock with master password
    pub fn unlock(&mut self, password: &str) -> Result<bool> {
        if self.master.verify(password)? {
            let key = self.master.derive_encryption_key(password)?;
            self.encryption_key = Some(key);
            self.unlocked = true;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Lock the credential manager (clear encryption key from memory)
    pub fn lock(&mut self) {
        if let Some(ref mut key) = self.encryption_key {
            // Securely zero out the key
            use zeroize::Zeroize;
            key.zeroize();
        }
        self.encryption_key = None;
        self.unlocked = false;
    }

    /// Save a password for a host
    pub async fn save_password(&mut self, host_id: Uuid, password: &str) -> Result<()> {
        let key = self
            .encryption_key
            .ok_or_else(|| anyhow::anyhow!("Master password not unlocked"))?;

        self.vault.store(host_id, password, &key)?;
        self.vault.save().await?;
        Ok(())
    }

    /// Get a saved password for a host
    pub fn get_password(&self, host_id: Uuid) -> Result<Option<String>> {
        let key = self
            .encryption_key
            .ok_or_else(|| anyhow::anyhow!("Master password not unlocked"))?;

        self.vault.retrieve(host_id, &key)
    }

    /// Check if a host has a saved password
    pub fn has_saved_password(&self, host_id: Uuid) -> bool {
        self.vault.has_credential(host_id)
    }

    /// Delete a saved password for a host
    pub async fn delete_password(&mut self, host_id: Uuid) -> Result<()> {
        self.vault.remove(host_id);
        self.vault.save().await?;
        Ok(())
    }

    /// Get list of hosts with saved passwords
    pub fn hosts_with_passwords(&self) -> Vec<Uuid> {
        self.vault.list_hosts()
    }
}

impl Drop for CredentialManager {
    fn drop(&mut self) {
        self.lock();
    }
}
