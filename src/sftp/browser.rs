//! SFTP file browser

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// File entry in the browser
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// File name
    pub name: String,
    /// Full path
    pub path: PathBuf,
    /// Whether this is a directory
    pub is_dir: bool,
    /// File size in bytes
    pub size: u64,
    /// Last modification time
    pub modified: Option<DateTime<Utc>>,
    /// Unix permissions (if available)
    pub permissions: Option<u32>,
    /// Whether entry is selected
    #[serde(skip)]
    pub selected: bool,
    /// Symlink target (if symlink)
    pub symlink_target: Option<PathBuf>,
}

impl FileEntry {
    /// Create a new file entry
    pub fn new(name: String, path: PathBuf, is_dir: bool, size: u64) -> Self {
        Self {
            name,
            path,
            is_dir,
            size,
            modified: None,
            permissions: None,
            selected: false,
            symlink_target: None,
        }
    }

    /// Create parent directory entry (..)
    pub fn parent(path: PathBuf) -> Self {
        Self {
            name: "..".to_string(),
            path,
            is_dir: true,
            size: 0,
            modified: None,
            permissions: None,
            selected: false,
            symlink_target: None,
        }
    }

    /// Format size for display
    pub fn size_display(&self) -> String {
        if self.is_dir {
            "<DIR>".to_string()
        } else {
            format_size(self.size)
        }
    }

    /// Format permissions for display (Unix style)
    pub fn permissions_display(&self) -> String {
        if let Some(perms) = self.permissions {
            format_permissions(perms)
        } else {
            "----------".to_string()
        }
    }
}

/// Format file size to human-readable
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1}T", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

/// Format Unix permissions to string
fn format_permissions(perms: u32) -> String {
    let mut s = String::with_capacity(10);
    
    // File type
    s.push(if perms & 0o40000 != 0 { 'd' } else { '-' });
    
    // Owner
    s.push(if perms & 0o400 != 0 { 'r' } else { '-' });
    s.push(if perms & 0o200 != 0 { 'w' } else { '-' });
    s.push(if perms & 0o100 != 0 { 'x' } else { '-' });
    
    // Group
    s.push(if perms & 0o040 != 0 { 'r' } else { '-' });
    s.push(if perms & 0o020 != 0 { 'w' } else { '-' });
    s.push(if perms & 0o010 != 0 { 'x' } else { '-' });
    
    // Other
    s.push(if perms & 0o004 != 0 { 'r' } else { '-' });
    s.push(if perms & 0o002 != 0 { 'w' } else { '-' });
    s.push(if perms & 0o001 != 0 { 'x' } else { '-' });
    
    s
}

/// Sort order for file listing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Name,
    NameDesc,
    Size,
    SizeDesc,
    Modified,
    ModifiedDesc,
}

impl SortOrder {
    /// Get next sort order (for cycling)
    pub fn next(&self) -> Self {
        match self {
            SortOrder::Name => SortOrder::NameDesc,
            SortOrder::NameDesc => SortOrder::Size,
            SortOrder::Size => SortOrder::SizeDesc,
            SortOrder::SizeDesc => SortOrder::Modified,
            SortOrder::Modified => SortOrder::ModifiedDesc,
            SortOrder::ModifiedDesc => SortOrder::Name,
        }
    }
}

/// Which pane is active
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaneSide {
    #[default]
    Left,
    Right,
}

/// Single file pane (local or remote)
pub struct FilePane {
    /// Current directory path
    pub path: PathBuf,
    /// Directory entries
    pub entries: Vec<FileEntry>,
    /// Currently highlighted entry index
    pub cursor: usize,
    /// Scroll offset for display
    pub scroll_offset: usize,
    /// Filter string
    pub filter: String,
    /// Sort order
    pub sort: SortOrder,
    /// Whether pane shows hidden files
    pub show_hidden: bool,
    /// Whether this is remote (SFTP) pane
    pub is_remote: bool,
}

impl FilePane {
    /// Create a new file pane
    pub fn new(path: PathBuf, is_remote: bool) -> Self {
        Self {
            path,
            entries: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            filter: String::new(),
            sort: SortOrder::default(),
            show_hidden: false,
            is_remote,
        }
    }

    /// Get filtered entries (respecting filter and hidden files)
    pub fn filtered_entries(&self) -> Vec<&FileEntry> {
        self.entries
            .iter()
            .filter(|e| {
                // Always show parent directory
                if e.name == ".." {
                    return true;
                }
                // Filter hidden files
                if !self.show_hidden && e.name.starts_with('.') {
                    return false;
                }
                // Apply text filter
                if !self.filter.is_empty() {
                    return e.name.to_lowercase().contains(&self.filter.to_lowercase());
                }
                true
            })
            .collect()
    }

    /// Move cursor up
    pub fn cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor down
    pub fn cursor_down(&mut self) {
        let max = self.filtered_entries().len().saturating_sub(1);
        if self.cursor < max {
            self.cursor += 1;
        }
    }

    /// Move cursor to top
    pub fn cursor_top(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to bottom
    pub fn cursor_bottom(&mut self) {
        self.cursor = self.filtered_entries().len().saturating_sub(1);
    }

    /// Page up
    pub fn page_up(&mut self, page_size: usize) {
        self.cursor = self.cursor.saturating_sub(page_size);
    }

    /// Page down
    pub fn page_down(&mut self, page_size: usize) {
        let max = self.filtered_entries().len().saturating_sub(1);
        self.cursor = (self.cursor + page_size).min(max);
    }

    /// Get currently selected entry
    pub fn current_entry(&self) -> Option<&FileEntry> {
        self.filtered_entries().get(self.cursor).copied()
    }

    /// Toggle selection on current entry
    pub fn toggle_selection(&mut self) {
        if let Some(idx) = self.real_index(self.cursor) {
            self.entries[idx].selected = !self.entries[idx].selected;
        }
    }

    /// Get real index from filtered index
    fn real_index(&self, filtered_idx: usize) -> Option<usize> {
        let filtered = self.filtered_entries();
        if filtered_idx < filtered.len() {
            let entry = filtered[filtered_idx];
            self.entries.iter().position(|e| std::ptr::eq(e, entry))
        } else {
            None
        }
    }

    /// Get all selected entries
    pub fn selected_entries(&self) -> Vec<&FileEntry> {
        self.entries.iter().filter(|e| e.selected).collect()
    }

    /// Clear all selections
    pub fn clear_selection(&mut self) {
        for entry in &mut self.entries {
            entry.selected = false;
        }
    }

    /// Sort entries
    pub fn sort_entries(&mut self) {
        let sort = self.sort;
        self.entries.sort_by(|a, b| {
            // Parent directory always first
            if a.name == ".." {
                return std::cmp::Ordering::Less;
            }
            if b.name == ".." {
                return std::cmp::Ordering::Greater;
            }
            // Directories before files
            match (a.is_dir, b.is_dir) {
                (true, false) => return std::cmp::Ordering::Less,
                (false, true) => return std::cmp::Ordering::Greater,
                _ => {}
            }
            // Apply sort order
            match sort {
                SortOrder::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortOrder::NameDesc => b.name.to_lowercase().cmp(&a.name.to_lowercase()),
                SortOrder::Size => a.size.cmp(&b.size),
                SortOrder::SizeDesc => b.size.cmp(&a.size),
                SortOrder::Modified => a.modified.cmp(&b.modified),
                SortOrder::ModifiedDesc => b.modified.cmp(&a.modified),
            }
        });
    }

    /// Load local directory
    pub async fn load_local(&mut self) -> Result<()> {
        self.entries.clear();
        self.cursor = 0;

        // Add parent directory
        if let Some(parent) = self.path.parent() {
            self.entries.push(FileEntry::parent(parent.to_path_buf()));
        }

        // Read directory
        let mut dir = tokio::fs::read_dir(&self.path).await?;
        while let Some(entry) = dir.next_entry().await? {
            let metadata = entry.metadata().await?;
            let file_entry = FileEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                path: entry.path(),
                is_dir: metadata.is_dir(),
                size: metadata.len(),
                modified: metadata.modified().ok().map(|t| DateTime::from(t)),
                permissions: {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        Some(metadata.permissions().mode())
                    }
                    #[cfg(not(unix))]
                    None
                },
                selected: false,
                symlink_target: if metadata.is_symlink() {
                    tokio::fs::read_link(entry.path()).await.ok()
                } else {
                    None
                },
            };
            self.entries.push(file_entry);
        }

        self.sort_entries();
        Ok(())
    }

    /// Navigate into a directory
    pub async fn enter_directory(&mut self) -> Result<bool> {
        if let Some(entry) = self.current_entry() {
            if entry.is_dir {
                self.path = entry.path.clone();
                if self.is_remote {
                    // Remote loading will be handled by caller with SFTP session
                    Ok(true)
                } else {
                    self.load_local().await?;
                    Ok(true)
                }
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    /// Load remote directory using SFTP session
    pub fn load_remote(&mut self, sftp_session: &super::sftp_session::SftpSession) -> Result<()> {
        self.entries = sftp_session.read_dir(&self.path)?;
        self.cursor = 0;
        self.sort_entries();
        Ok(())
    }

    /// Navigate to parent directory
    pub fn go_parent(&mut self) -> bool {
        if let Some(parent) = self.path.parent() {
            if parent != self.path {
                self.path = parent.to_path_buf();
                self.cursor = 0;
                return true;
            }
        }
        false
    }

    /// Set the filter string
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.cursor = 0;
    }

    /// Cycle to next sort order
    pub fn cycle_sort(&mut self) {
        self.sort = self.sort.next();
        self.sort_entries();
    }

    /// Toggle hidden files visibility
    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.cursor = 0;
    }
}

/// Dual-pane file browser
pub struct FileBrowser {
    /// Left pane (typically local)
    pub left: FilePane,
    /// Right pane (typically remote)
    pub right: FilePane,
    /// Active pane
    pub active: PaneSide,
}

impl FileBrowser {
    /// Create a new file browser
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        
        Self {
            left: FilePane::new(home.clone(), false),
            right: FilePane::new(PathBuf::from("/"), true),
            active: PaneSide::Left,
        }
    }

    /// Get active pane
    pub fn active_pane(&self) -> &FilePane {
        match self.active {
            PaneSide::Left => &self.left,
            PaneSide::Right => &self.right,
        }
    }

    /// Get mutable active pane
    pub fn active_pane_mut(&mut self) -> &mut FilePane {
        match self.active {
            PaneSide::Left => &mut self.left,
            PaneSide::Right => &mut self.right,
        }
    }

    /// Get inactive pane
    pub fn inactive_pane(&self) -> &FilePane {
        match self.active {
            PaneSide::Left => &self.right,
            PaneSide::Right => &self.left,
        }
    }

    /// Switch active pane
    pub fn switch_pane(&mut self) {
        self.active = match self.active {
            PaneSide::Left => PaneSide::Right,
            PaneSide::Right => PaneSide::Left,
        };
    }

    /// Initialize the browser
    pub async fn init(&mut self) -> Result<()> {
        self.left.load_local().await?;
        // Remote pane will be loaded when SFTP connection is established
        Ok(())
    }
}

impl Default for FileBrowser {
    fn default() -> Self {
        Self::new()
    }
}
