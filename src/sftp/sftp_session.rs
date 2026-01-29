//! SFTP session management
//!
//! Manages SFTP sessions that can share authentication with SSH connections.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use ssh2::{FileStat, Sftp};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::browser::FileEntry;

/// SFTP session wrapping ssh2::Sftp with metadata
pub struct SftpSession {
    /// Session ID
    pub id: Uuid,
    /// Host ID this session belongs to
    pub host_id: Uuid,
    /// SSH connection ID used for this SFTP session
    pub connection_id: Uuid,
    /// The underlying SFTP handle
    sftp: Sftp,
    /// Session creation time
    pub created_at: DateTime<Utc>,
    /// Current remote working directory
    pub cwd: PathBuf,
}

impl SftpSession {
    /// Create a new SFTP session from an existing SSH connection
    /// Uses username to determine home directory
    pub fn new(sftp: Sftp, host_id: Uuid, connection_id: Uuid, username: &str) -> Result<Self> {
        // Try to get the home directory in order of preference:
        // 1. realpath(".") - works on most servers
        // 2. /home/{username} - common Linux layout
        // 3. / - fallback
        let cwd = sftp
            .realpath(Path::new("."))
            .or_else(|_| {
                let home_path = PathBuf::from(format!("/home/{}", username));
                // Verify the path exists
                if sftp.stat(&home_path).is_ok() {
                    Ok(home_path)
                } else {
                    Err(anyhow!("Home directory not found"))
                }
            })
            .unwrap_or_else(|_| PathBuf::from("/"));

        Ok(Self {
            id: Uuid::new_v4(),
            host_id,
            connection_id,
            sftp,
            created_at: Utc::now(),
            cwd,
        })
    }

    /// Get the underlying SFTP handle
    pub fn sftp(&self) -> &Sftp {
        &self.sftp
    }

    /// Read a remote directory and return file entries
    pub fn read_dir(&self, path: &Path) -> Result<Vec<FileEntry>> {
        let mut entries = Vec::new();

        // Add parent directory entry if not at root
        if path.parent().is_some() && path != Path::new("/") {
            let parent_path = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("/"));
            entries.push(FileEntry::parent(parent_path));
        }

        // Read directory contents
        let dir_contents = self.sftp.readdir(path)?;

        for (file_path, stat) in dir_contents {
            let name = file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| file_path.to_string_lossy().to_string());

            // Skip . and .. entries (we add our own parent entry)
            if name == "." || name == ".." {
                continue;
            }

            let entry = file_stat_to_entry(name, file_path, &stat);
            entries.push(entry);
        }

        Ok(entries)
    }

    /// Get file info
    pub fn stat(&self, path: &Path) -> Result<FileStat> {
        self.sftp
            .stat(path)
            .map_err(|e| anyhow!("Failed to stat {}: {}", path.display(), e))
    }

    /// Get real path (resolve symlinks)
    pub fn realpath(&self, path: &Path) -> Result<PathBuf> {
        self.sftp
            .realpath(path)
            .map_err(|e| anyhow!("Failed to realpath {}: {}", path.display(), e))
    }

    /// Create a directory
    pub fn mkdir(&self, path: &Path, mode: i32) -> Result<()> {
        self.sftp
            .mkdir(path, mode)
            .map_err(|e| anyhow!("Failed to mkdir {}: {}", path.display(), e))
    }

    /// Remove a directory (not exposed in remote UI per safety requirements)
    pub fn rmdir(&self, path: &Path) -> Result<()> {
        self.sftp
            .rmdir(path)
            .map_err(|e| anyhow!("Failed to rmdir {}: {}", path.display(), e))
    }

    /// Remove a file (not exposed in remote UI per safety requirements)  
    pub fn unlink(&self, path: &Path) -> Result<()> {
        self.sftp
            .unlink(path)
            .map_err(|e| anyhow!("Failed to unlink {}: {}", path.display(), e))
    }

    /// Rename a file or directory
    pub fn rename(&self, src: &Path, dst: &Path) -> Result<()> {
        self.sftp.rename(src, dst, None).map_err(|e| {
            anyhow!(
                "Failed to rename {} to {}: {}",
                src.display(),
                dst.display(),
                e
            )
        })
    }

    /// Open a file for reading (for downloads)
    pub fn open_read(&self, path: &Path) -> Result<ssh2::File> {
        self.sftp
            .open(path)
            .map_err(|e| anyhow!("Failed to open {}: {}", path.display(), e))
    }

    /// Create a file for writing (for uploads)
    pub fn create(&self, path: &Path) -> Result<ssh2::File> {
        self.sftp
            .create(path)
            .map_err(|e| anyhow!("Failed to create {}: {}", path.display(), e))
    }
}

/// Convert ssh2::FileStat to our FileEntry
fn file_stat_to_entry(name: String, path: PathBuf, stat: &FileStat) -> FileEntry {
    let is_dir = stat.is_dir();
    let size = stat.size.unwrap_or(0);

    let modified = stat
        .mtime
        .map(|t| DateTime::from_timestamp(t as i64, 0).unwrap_or_default());

    let permissions = stat.perm;

    FileEntry {
        name,
        path,
        is_dir,
        size,
        modified,
        permissions,
        selected: false,
        symlink_target: None, // ssh2 doesn't easily expose symlink target
    }
}

/// Manager for multiple SFTP sessions
pub struct SftpSessionManager {
    sessions: HashMap<Uuid, SftpSession>,
}

impl SftpSessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Add a session to the manager
    pub fn add(&mut self, session: SftpSession) -> Uuid {
        let id = session.id;
        self.sessions.insert(id, session);
        id
    }

    /// Get a session by ID
    pub fn get(&self, id: Uuid) -> Option<&SftpSession> {
        self.sessions.get(&id)
    }

    /// Get a mutable session by ID
    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut SftpSession> {
        self.sessions.get_mut(&id)
    }

    /// Get a session by host ID
    pub fn get_by_host(&self, host_id: Uuid) -> Option<&SftpSession> {
        self.sessions.values().find(|s| s.host_id == host_id)
    }

    /// Get a mutable session by host ID
    pub fn get_by_host_mut(&mut self, host_id: Uuid) -> Option<&mut SftpSession> {
        self.sessions.values_mut().find(|s| s.host_id == host_id)
    }

    /// Remove a session
    pub fn remove(&mut self, id: Uuid) -> Option<SftpSession> {
        self.sessions.remove(&id)
    }

    /// Remove all sessions for a host
    pub fn remove_by_host(&mut self, host_id: Uuid) {
        self.sessions.retain(|_, s| s.host_id != host_id);
    }

    /// List all sessions
    pub fn list(&self) -> Vec<&SftpSession> {
        self.sessions.values().collect()
    }

    /// Get session count
    pub fn count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for SftpSessionManager {
    fn default() -> Self {
        Self::new()
    }
}
