//! Master password handling
//!
//! Uses Argon2id for:
//! - Password verification (hash stored in OS keyring or fallback file)
//! - Key derivation for encryption

use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::KEYRING_SERVICE;

/// Fallback file storage for master password hash when keyring isn't available
#[derive(Debug, Serialize, Deserialize, Default)]
struct MasterPasswordFile {
    /// Argon2 hash of master password
    hash: Option<String>,
    /// Salt for key derivation (base64 encoded)
    kdf_salt: Option<String>,
}

impl MasterPasswordFile {
    fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rustyssh")
            .join(".master_key")
    }

    fn load() -> Self {
        if let Ok(content) = std::fs::read_to_string(Self::path()) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }

        Ok(())
    }
}

/// Master password handler
pub struct MasterPassword {
    /// Cached hash for verification (loaded from keyring or file)
    cached_hash: Option<String>,
    /// Salt for key derivation (separate from verification salt)
    kdf_salt: Option<[u8; 32]>,
    /// Whether we're using file-based storage (keyring not available)
    use_file_storage: bool,
}

impl MasterPassword {
    /// Create a new master password handler
    pub fn new() -> Self {
        let mut instance = Self {
            cached_hash: None,
            kdf_salt: None,
            use_file_storage: false,
        };

        // Try to load existing hash from keyring, fall back to file
        instance.load_from_storage();

        instance
    }

    /// Check if a master password has been set
    pub fn is_set(&self) -> bool {
        self.cached_hash.is_some()
    }

    /// Load master password hash from OS keyring or fallback file
    fn load_from_storage(&mut self) {
        // Try keyring first
        if self.try_load_from_keyring() {
            return;
        }

        // Fall back to file storage
        self.use_file_storage = true;
        self.load_from_file();
    }

    fn try_load_from_keyring(&mut self) -> bool {
        let hash_entry = match keyring::Entry::new(KEYRING_SERVICE, "master_hash") {
            Ok(e) => e,
            Err(_) => return false,
        };

        if let Ok(hash) = hash_entry.get_password() {
            self.cached_hash = Some(hash);

            // Also load KDF salt
            if let Ok(salt_entry) = keyring::Entry::new(KEYRING_SERVICE, "kdf_salt") {
                if let Ok(salt_b64) = salt_entry.get_password() {
                    use base64::Engine;
                    if let Ok(salt_bytes) =
                        base64::engine::general_purpose::STANDARD.decode(&salt_b64)
                    {
                        if salt_bytes.len() == 32 {
                            let mut salt = [0u8; 32];
                            salt.copy_from_slice(&salt_bytes);
                            self.kdf_salt = Some(salt);
                        }
                    }
                }
            }

            return true;
        }

        false
    }

    fn load_from_file(&mut self) {
        let file = MasterPasswordFile::load();

        if let Some(hash) = file.hash {
            self.cached_hash = Some(hash);
        }

        if let Some(salt_b64) = file.kdf_salt {
            use base64::Engine;
            if let Ok(salt_bytes) = base64::engine::general_purpose::STANDARD.decode(&salt_b64) {
                if salt_bytes.len() == 32 {
                    let mut salt = [0u8; 32];
                    salt.copy_from_slice(&salt_bytes);
                    self.kdf_salt = Some(salt);
                }
            }
        }
    }

    /// Set up a new master password
    pub fn setup(&mut self, password: &str) -> Result<()> {
        // Generate salt for verification hash
        let salt = SaltString::generate(&mut OsRng);

        // Create Argon2id hasher with OWASP recommended params
        // m=19456 (19MB), t=2, p=1
        let params = Params::new(19456, 2, 1, Some(32))
            .map_err(|e| anyhow!("Failed to create Argon2 params: {}", e))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        // Hash the password
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow!("Failed to hash password: {}", e))?
            .to_string();

        // Generate separate salt for key derivation
        let mut kdf_salt = [0u8; 32];
        use rand::RngCore;
        OsRng.fill_bytes(&mut kdf_salt);

        // Try to store in keyring first
        if !self.use_file_storage {
            match self.try_store_in_keyring(&hash, &kdf_salt) {
                Ok(()) => {
                    self.cached_hash = Some(hash);
                    self.kdf_salt = Some(kdf_salt);
                    return Ok(());
                }
                Err(_) => {
                    // Keyring failed, fall back to file
                    self.use_file_storage = true;
                }
            }
        }

        // Store in file
        self.store_in_file(&hash, &kdf_salt)?;
        self.cached_hash = Some(hash);
        self.kdf_salt = Some(kdf_salt);

        Ok(())
    }

    fn try_store_in_keyring(&self, hash: &str, kdf_salt: &[u8; 32]) -> Result<()> {
        let hash_entry = keyring::Entry::new(KEYRING_SERVICE, "master_hash")
            .map_err(|e| anyhow!("Failed to create keyring entry: {}", e))?;
        hash_entry
            .set_password(hash)
            .map_err(|e| anyhow!("Failed to store in keyring: {}", e))?;

        use base64::Engine;
        let salt_b64 = base64::engine::general_purpose::STANDARD.encode(kdf_salt);
        let salt_entry = keyring::Entry::new(KEYRING_SERVICE, "kdf_salt")
            .map_err(|e| anyhow!("Failed to create keyring entry: {}", e))?;
        salt_entry
            .set_password(&salt_b64)
            .map_err(|e| anyhow!("Failed to store in keyring: {}", e))?;

        Ok(())
    }

    fn store_in_file(&self, hash: &str, kdf_salt: &[u8; 32]) -> Result<()> {
        use base64::Engine;
        let salt_b64 = base64::engine::general_purpose::STANDARD.encode(kdf_salt);

        let file = MasterPasswordFile {
            hash: Some(hash.to_string()),
            kdf_salt: Some(salt_b64),
        };

        file.save()
    }

    /// Verify the master password
    pub fn verify(&self, password: &str) -> Result<bool> {
        let hash_str = self
            .cached_hash
            .as_ref()
            .ok_or_else(|| anyhow!("No master password set"))?;

        let parsed_hash = PasswordHash::new(hash_str)
            .map_err(|e| anyhow!("Failed to parse stored hash: {}", e))?;

        // Use same params for verification
        let params = Params::new(19456, 2, 1, Some(32))
            .map_err(|e| anyhow!("Failed to create Argon2 params: {}", e))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        Ok(argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    /// Derive an encryption key from the master password
    pub fn derive_encryption_key(&self, password: &str) -> Result<[u8; 32]> {
        let kdf_salt = self
            .kdf_salt
            .ok_or_else(|| anyhow!("No KDF salt available"))?;

        // Use Argon2id for key derivation with same security params
        let params = Params::new(19456, 2, 1, Some(32))
            .map_err(|e| anyhow!("Failed to create Argon2 params: {}", e))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut key = [0u8; 32];
        argon2
            .hash_password_into(password.as_bytes(), &kdf_salt, &mut key)
            .map_err(|e| anyhow!("Failed to derive encryption key: {}", e))?;

        Ok(key)
    }
}

impl Default for MasterPassword {
    fn default() -> Self {
        Self::new()
    }
}
