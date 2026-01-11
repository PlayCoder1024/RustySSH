//! SSH key management

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// SSH key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfo {
    /// Key file path
    pub path: PathBuf,
    /// Key type (ed25519, rsa, etc.)
    pub key_type: String,
    /// Key fingerprint
    pub fingerprint: String,
    /// Comment from public key
    pub comment: String,
    /// Whether key is encrypted with passphrase
    pub encrypted: bool,
    /// Associated public key path
    pub public_key_path: Option<PathBuf>,
}

/// Key manager for SSH keys
pub struct KeyManager {
    /// Default SSH directory
    ssh_dir: PathBuf,
    /// Cached key information
    keys: HashMap<PathBuf, KeyInfo>,
}

impl KeyManager {
    /// Create a new key manager
    pub fn new() -> Self {
        let ssh_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".ssh");

        Self {
            ssh_dir,
            keys: HashMap::new(),
        }
    }

    /// Get SSH directory path
    pub fn ssh_dir(&self) -> &Path {
        &self.ssh_dir
    }

    /// Scan SSH directory for keys
    pub fn scan_keys(&mut self) -> Result<Vec<KeyInfo>> {
        self.keys.clear();

        if !self.ssh_dir.exists() {
            return Ok(vec![]);
        }

        let mut keys = Vec::new();

        for entry in fs::read_dir(&self.ssh_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Skip directories and public keys
            if path.is_dir() || path.extension().map_or(false, |e| e == "pub") {
                continue;
            }

            // Try to identify as a key file
            if let Some(key_info) = self.identify_key(&path) {
                self.keys.insert(path.clone(), key_info.clone());
                keys.push(key_info);
            }
        }

        Ok(keys)
    }

    /// Identify a key file
    fn identify_key(&self, path: &Path) -> Option<KeyInfo> {
        // Try to read first line to check if it's a key
        let content = fs::read_to_string(path).ok()?;
        let first_line = content.lines().next()?;

        // Check for key file markers
        let (key_type, encrypted) = if first_line.contains("OPENSSH PRIVATE KEY") {
            // Modern OpenSSH format
            let encrypted = content.contains("ENCRYPTED");
            ("openssh".to_string(), encrypted)
        } else if first_line.contains("RSA PRIVATE KEY") {
            let encrypted = content.contains("ENCRYPTED") || content.contains("Proc-Type: 4,ENCRYPTED");
            ("rsa".to_string(), encrypted)
        } else if first_line.contains("EC PRIVATE KEY") {
            let encrypted = content.contains("ENCRYPTED");
            ("ecdsa".to_string(), encrypted)
        } else if first_line.contains("DSA PRIVATE KEY") {
            let encrypted = content.contains("ENCRYPTED");
            ("dsa".to_string(), encrypted)
        } else {
            return None;
        };

        // Try to get fingerprint from public key
        let pub_path = path.with_extension("pub");
        let (fingerprint, comment) = if pub_path.exists() {
            self.parse_public_key(&pub_path).unwrap_or_default()
        } else {
            (String::new(), String::new())
        };

        Some(KeyInfo {
            path: path.to_path_buf(),
            key_type,
            fingerprint,
            comment,
            encrypted,
            public_key_path: if pub_path.exists() { Some(pub_path) } else { None },
        })
    }

    /// Parse public key file for fingerprint and comment
    fn parse_public_key(&self, path: &Path) -> Option<(String, String)> {
        let content = fs::read_to_string(path).ok()?;
        let parts: Vec<&str> = content.split_whitespace().collect();

        if parts.len() >= 2 {
            // Get fingerprint using ssh-keygen if available
            let fingerprint = self.get_fingerprint(path).unwrap_or_else(|| {
                // Fallback: just show first 20 chars of the key data
                format!("...{}", &parts[1][..20.min(parts[1].len())])
            });
            let comment = parts.get(2).map(|s| s.to_string()).unwrap_or_default();
            return Some((fingerprint, comment));
        }

        None
    }

    /// Get key fingerprint using ssh-keygen
    fn get_fingerprint(&self, pub_key_path: &Path) -> Option<String> {
        let output = Command::new("ssh-keygen")
            .args(["-l", "-f"])
            .arg(pub_key_path)
            .output()
            .ok()?;
        
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Format: "256 SHA256:xxxx comment (ED25519)"
            let parts: Vec<&str> = stdout.split_whitespace().collect();
            if parts.len() >= 2 {
                return Some(parts[1].to_string());
            }
        }
        None
    }

    /// Generate a new Ed25519 key pair using ssh-keygen
    pub fn generate_ed25519(&self, path: &Path, comment: &str, passphrase: Option<&str>) -> Result<()> {
        self.generate_key(path, "ed25519", comment, passphrase)
    }

    /// Generate a new RSA key pair using ssh-keygen
    pub fn generate_rsa(&self, path: &Path, bits: usize, comment: &str, passphrase: Option<&str>) -> Result<()> {
        let mut cmd = Command::new("ssh-keygen");
        cmd.args(["-t", "rsa"])
            .args(["-b", &bits.to_string()])
            .args(["-f"])
            .arg(path)
            .args(["-C", comment])
            .args(["-N", passphrase.unwrap_or("")]);
        
        let output = cmd.output()
            .map_err(|e| anyhow!("Failed to run ssh-keygen: {}", e))?;
        
        if !output.status.success() {
            return Err(anyhow!("ssh-keygen failed: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }
        
        Ok(())
    }

    /// Generate a key with specified type
    fn generate_key(&self, path: &Path, key_type: &str, comment: &str, passphrase: Option<&str>) -> Result<()> {
        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut cmd = Command::new("ssh-keygen");
        cmd.args(["-t", key_type])
            .args(["-f"])
            .arg(path)
            .args(["-C", comment])
            .args(["-N", passphrase.unwrap_or("")]);
        
        let output = cmd.output()
            .map_err(|e| anyhow!("Failed to run ssh-keygen: {}", e))?;
        
        if !output.status.success() {
            return Err(anyhow!("ssh-keygen failed: {}", 
                String::from_utf8_lossy(&output.stderr)));
        }
        
        Ok(())
    }

    /// Get list of cached keys
    pub fn list_keys(&self) -> Vec<&KeyInfo> {
        self.keys.values().collect()
    }

    /// Read public key content
    pub fn read_public_key(&self, path: &Path) -> Result<String> {
        let pub_path = if path.extension().map_or(false, |e| e == "pub") {
            path.to_path_buf()
        } else {
            path.with_extension("pub")
        };
        
        fs::read_to_string(&pub_path)
            .map_err(|e| anyhow!("Failed to read public key: {}", e))
    }

    /// Delete a key pair
    pub fn delete_key(&mut self, path: &Path) -> Result<()> {
        // Delete private key
        if path.exists() {
            fs::remove_file(path)?;
        }
        
        // Delete public key
        let pub_path = path.with_extension("pub");
        if pub_path.exists() {
            fs::remove_file(&pub_path)?;
        }
        
        // Remove from cache
        self.keys.remove(path);
        
        Ok(())
    }
}

impl Default for KeyManager {
    fn default() -> Self {
        Self::new()
    }
}
