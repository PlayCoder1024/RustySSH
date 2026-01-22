//! Application state management

use super::{AppEvent, EventHandler};
use crate::config::{Config, HostConfig};
use crate::credentials::CredentialManager;
use crate::sftp::{FileBrowser, SftpSession, SftpSessionManager, TransferQueue};
use crate::ssh::{ConnectionPool, SessionManager, SshConnection, ProxyConnection};
use crate::tui::{Icons, Theme, Tui};
use crate::tui::terminal_render::render_screen_to_lines;
use crate::tui::highlight::{highlight_styled_line, TerminalHighlightConfig};
use anyhow::{anyhow, Result};
use crossterm::event::{KeyCode, KeyModifiers, MouseEvent, MouseEventKind};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Current application view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Connections,
    Session,
    Sftp,
    Tunnels,
    Keys,
    Settings,
    Help,
}

/// Application running state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppState {
    #[default]
    Running,
    Quit,
}

/// Session info for rendering (avoids borrow conflicts)
#[derive(Clone)]
pub struct SessionInfo {
    pub id: Uuid,
    pub name: String,
    /// Styled terminal lines with full color support
    pub styled_lines: Vec<ratatui::text::Line<'static>>,
    pub cursor_position: (u16, u16),
    pub cursor_visible: bool,
}

/// Render state snapshot (avoids borrow conflicts in draw callback)
#[derive(Clone)]
pub struct RenderState {
    pub view: View,
    pub theme: Theme,
    pub icons: Icons,
    pub config: Config,
    pub sessions: Vec<SessionInfo>,
    pub active_session: Option<Uuid>,
    pub status_message: Option<String>,
    pub selected_host_index: usize,
    pub host_count: usize,
    /// File browser snapshot for SFTP view
    pub file_browser: Option<FileBrowserSnapshot>,
    /// Transfer queue snapshot for SFTP view
    pub transfer_info: TransferQueueSnapshot,
}

/// Snapshot of a file pane for rendering
#[derive(Clone, Default)]
pub struct FilePaneSnapshot {
    pub path: String,
    pub entries: Vec<FileEntrySnapshot>,
    pub cursor: usize,
    pub is_remote: bool,
}

/// Snapshot of a file entry for rendering
#[derive(Clone)]
pub struct FileEntrySnapshot {
    pub name: String,
    pub is_dir: bool,
    pub size_display: String,
    pub selected: bool,
}

/// Snapshot of file browser for rendering
#[derive(Clone, Default)]
pub struct FileBrowserSnapshot {
    pub left: FilePaneSnapshot,
    pub right: FilePaneSnapshot,
    pub active_is_left: bool,
}

/// Snapshot of transfer queue for rendering
#[derive(Clone, Default)]
pub struct TransferQueueSnapshot {
    pub pending_count: usize,
    pub active_count: usize,
    pub active_transfers: Vec<TransferItemSnapshot>,
}

/// Snapshot of a transfer item
#[derive(Clone)]
pub struct TransferItemSnapshot {
    pub filename: String,
    pub progress: f64,
    pub speed_display: String,
    pub eta_display: String,
    pub is_upload: bool,
}

/// Active SSH channel for a session
pub struct ActiveChannel {
    pub session_id: Uuid,
    pub connection_id: Uuid,
    /// Channel for sending data to SSH
    pub input_tx: mpsc::UnboundedSender<Vec<u8>>,
}

/// Main application struct
pub struct App {
    /// Current view
    pub view: View,
    /// Application state
    pub state: AppState,
    /// Configuration
    pub config: Config,
    /// SSH session manager (terminal emulation)
    pub sessions: SessionManager,
    /// SSH connection pool
    pub connections: ConnectionPool,
    /// Active channels mapped by session ID
    pub channels: HashMap<Uuid, ActiveChannel>,
    /// Active session ID (if in session view)
    pub active_session: Option<Uuid>,
    /// Status message
    pub status_message: Option<String>,
    /// Selected host index in connections view
    pub selected_host_index: usize,
    /// Terminal UI
    tui: Tui,
    /// Event handler
    events: EventHandler,
    /// Theme
    pub theme: Theme,
    /// Icons (Nerd Font or ASCII)
    pub icons: Icons,
    /// Credential manager for secure password storage
    pub credentials: CredentialManager,
    /// SFTP session manager
    pub sftp_sessions: SftpSessionManager,
    /// File browser for SFTP view
    pub file_browser: Option<FileBrowser>,
    /// Transfer queue for SFTP operations
    pub transfer_queue: TransferQueue,
    /// Active SFTP host ID (for which host the SFTP view is showing)
    pub active_sftp_host: Option<Uuid>,
    /// View history stack for back navigation
    pub view_history: Vec<View>,
    /// Session passwords for SFTP reuse (cleared on disconnect)
    session_passwords: HashMap<Uuid, String>,
}

impl App {
    /// Create a new application instance
    pub async fn new() -> Result<Self> {
        let config = Config::load().await?;
        let theme = Theme::default();
        let icons = Icons::detect();
        let tui = Tui::new()?;
        let events = EventHandler::new(Duration::from_millis(50));
        let credentials = CredentialManager::new().await?;

        Ok(Self {
            view: View::default(),
            state: AppState::default(),
            config,
            sessions: SessionManager::new(),
            connections: ConnectionPool::new(),
            channels: HashMap::new(),
            active_session: None,
            status_message: None,
            selected_host_index: 0,
            tui,
            events,
            theme,
            icons,
            credentials,
            sftp_sessions: SftpSessionManager::new(),
            file_browser: None,
            transfer_queue: TransferQueue::default(),
            active_sftp_host: None,
            view_history: Vec::new(),
            session_passwords: HashMap::new(),
        })
    }

    /// Get all hosts (groups + ungrouped)
    fn all_hosts(&self) -> Vec<&HostConfig> {
        let mut hosts: Vec<&HostConfig> = Vec::new();
        for group in &self.config.groups {
            if group.expanded {
                hosts.extend(group.hosts.iter());
            }
        }
        hosts.extend(self.config.hosts.iter());
        hosts
    }

    /// Get selected host
    fn selected_host(&self) -> Option<&HostConfig> {
        self.all_hosts().get(self.selected_host_index).copied()
    }

    /// Push current view to history and switch to new view
    fn push_view(&mut self, new_view: View) {
        // Don't push if same view
        if self.view != new_view {
            self.view_history.push(self.view);
            self.view = new_view;
        }
    }

    /// Pop view from history (go back)
    fn pop_view(&mut self) {
        if let Some(prev_view) = self.view_history.pop() {
            self.view = prev_view;
        } else {
            // Default to connections if no history
            self.view = View::Connections;
        }
    }

    /// Run the main application loop
    pub async fn run(&mut self) -> Result<()> {
        self.tui.enter()?;
        self.events.start();

        while self.state != AppState::Quit {
            // Create render state to avoid borrow conflict
            let render_state = RenderState {
                view: self.view,
                theme: self.theme.clone(),
                icons: self.icons.clone(),
                config: self.config.clone(),
                sessions: {
                    let highlight_config = TerminalHighlightConfig::default();
                    self.sessions
                        .list()
                        .iter()
                        .map(|s| {
                            let raw_lines = render_screen_to_lines(s.screen());
                            SessionInfo {
                                id: s.id,
                                name: s.name.clone(),
                                styled_lines: raw_lines
                                    .into_iter()
                                    .map(|line| highlight_styled_line(line, &highlight_config))
                                    .collect(),
                                cursor_position: s.cursor_position(),
                                cursor_visible: s.cursor_visible(),
                            }
                        })
                        .collect()
                },
                active_session: self.active_session,
                status_message: self.status_message.clone(),
                selected_host_index: self.selected_host_index,
                host_count: self.all_hosts().len(),
                file_browser: self.file_browser.as_ref().map(|browser| {
                    use crate::sftp::PaneSide;
                    FileBrowserSnapshot {
                        left: FilePaneSnapshot {
                            path: browser.left.path.display().to_string(),
                            entries: browser.left.filtered_entries().iter().map(|e| FileEntrySnapshot {
                                name: e.name.clone(),
                                is_dir: e.is_dir,
                                size_display: e.size_display(),
                                selected: e.selected,
                            }).collect(),
                            cursor: browser.left.cursor,
                            is_remote: browser.left.is_remote,
                        },
                        right: FilePaneSnapshot {
                            path: browser.right.path.display().to_string(),
                            entries: browser.right.filtered_entries().iter().map(|e| FileEntrySnapshot {
                                name: e.name.clone(),
                                is_dir: e.is_dir,
                                size_display: e.size_display(),
                                selected: e.selected,
                            }).collect(),
                            cursor: browser.right.cursor,
                            is_remote: browser.right.is_remote,
                        },
                        active_is_left: browser.active == PaneSide::Left,
                    }
                }),
                transfer_info: TransferQueueSnapshot {
                    pending_count: self.transfer_queue.pending().len(),
                    active_count: self.transfer_queue.active().len(),
                    active_transfers: self.transfer_queue.active().iter().map(|t| {
                        use crate::sftp::TransferDirection;
                        TransferItemSnapshot {
                            filename: t.source.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                            progress: t.progress(),
                            speed_display: t.speed_display(),
                            eta_display: t.eta_display(),
                            is_upload: t.direction == TransferDirection::Upload,
                        }
                    }).collect(),
                },
            };

            // Render UI
            self.tui.draw(|frame| {
                crate::tui::ui::render_with_state(frame, &render_state);
            })?;

            // Handle events
            if let Some(event) = self.events.next().await {
                self.handle_event(event).await?;
            }
        }

        self.tui.exit()?;
        Ok(())
    }

    /// Handle application events
    async fn handle_event(&mut self, event: AppEvent) -> Result<()> {
        match event {
            AppEvent::Key(key) => {
                // Handle Ctrl+C/Q - only quit from non-session views
                // In session view, forward Ctrl+C to remote server
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('c') => {
                            if self.view == View::Session {
                                // Forward Ctrl+C (0x03) to remote
                                if let Some(session_id) = self.active_session {
                                    if let Some(channel) = self.channels.get(&session_id) {
                                        let _ = channel.input_tx.send(vec![0x03]);
                                    }
                                }
                            } else {
                                self.state = AppState::Quit;
                            }
                        }
                        KeyCode::Char('q') => {
                            // Ctrl+Q always quits (escape hatch from session)
                            self.state = AppState::Quit;
                        }
                        _ => {
                            // Forward other Ctrl+key combos to session
                            if self.view == View::Session {
                                if let Some(session_id) = self.active_session {
                                    if let Some(channel) = self.channels.get(&session_id) {
                                        let data = self.key_to_bytes(&key);
                                        if !data.is_empty() {
                                            let _ = channel.input_tx.send(data);
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    self.handle_key(key).await?;
                }
            }
            AppEvent::Resize(w, h) => {
                // Handle terminal resize
                // Account for: status bar (2), tab bar (2), terminal block borders (2 rows, 2 cols)
                if let Some(session_id) = self.active_session {
                    let adjusted_cols = w.saturating_sub(2);
                    let adjusted_rows = h.saturating_sub(6);
                    self.sessions.resize_session(session_id, adjusted_cols, adjusted_rows).await?;
                }
            }
            AppEvent::SshData { session_id, data } => {
                self.sessions.process_data(session_id, &data).await?;
            }
            AppEvent::SshDisconnected { session_id, reason } => {
                self.status_message = Some(format!("Disconnected: {}", reason));
                self.channels.remove(&session_id);
                self.sessions.remove(session_id);
                if self.active_session == Some(session_id) {
                    self.view = View::Connections;
                    self.active_session = None;
                }
            }
            AppEvent::Error(msg) => {
                self.status_message = Some(format!("Error: {}", msg));
            }
            AppEvent::Mouse(mouse) => {
                self.handle_mouse(mouse).await?;
            }
            AppEvent::Tick | AppEvent::SftpProgress { .. } => {
                // Tick and SFTP events are handled elsewhere or ignored
            }
        }
        Ok(())
    }

    /// Handle key events based on current view
    async fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        // View-specific key handling
        match self.view {
            View::Connections => self.handle_connections_key(key).await?,
            View::Session => self.handle_session_key(key).await?,
            View::Sftp => self.handle_sftp_key(key).await?,
            View::Tunnels => self.handle_tunnels_key(key).await?,
            View::Keys => self.handle_keys_key(key).await?,
            View::Settings => self.handle_settings_key(key).await?,
            View::Help => self.handle_help_key(key).await?,
        }
        Ok(())
    }

    /// Handle mouse events for scrolling
    async fn handle_mouse(&mut self, mouse: MouseEvent) -> Result<()> {
        use MouseEventKind::*;
        
        match mouse.kind {
            ScrollUp => {
                match self.view {
                    View::Connections => {
                        // Scroll up in host list (move selection up)
                        if self.selected_host_index > 0 {
                            self.selected_host_index -= 1;
                        }
                    }
                    View::Session => {
                        // Scroll up in terminal history
                        if let Some(session_id) = self.active_session {
                            if let Some(session) = self.sessions.get_mut(session_id) {
                                session.scroll_up(3);
                            }
                        }
                    }
                    _ => {}
                }
            }
            ScrollDown => {
                match self.view {
                    View::Connections => {
                        // Scroll down in host list (move selection down)
                        let host_count = self.all_hosts().len();
                        if self.selected_host_index + 1 < host_count {
                            self.selected_host_index += 1;
                        }
                    }
                    View::Session => {
                        // Scroll down in terminal history (towards present)
                        if let Some(session_id) = self.active_session {
                            if let Some(session) = self.sessions.get_mut(session_id) {
                                session.scroll_down(3);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_connections_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        let host_count = self.all_hosts().len();

        match key.code {
            KeyCode::Char('q') => self.state = AppState::Quit,
            KeyCode::Char('?') => self.view = View::Help,
            KeyCode::Char('s') => self.view = View::Settings,
            KeyCode::Char('K') => self.view = View::Keys, // Shift+K for Keys view
            KeyCode::Char('t') => self.view = View::Tunnels,
            KeyCode::Char('f') => {
                // Open SFTP for selected host
                if let Some(host) = self.selected_host().cloned() {
                    self.open_sftp_for_host(host).await?;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_host_index > 0 {
                    self.selected_host_index -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_host_index + 1 < host_count {
                    self.selected_host_index += 1;
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected_host_index = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                if host_count > 0 {
                    self.selected_host_index = host_count - 1;
                }
            }
            KeyCode::Enter => {
                // Connect to selected host
                if let Some(host) = self.selected_host().cloned() {
                    self.connect_to_host(host).await?;
                }
            }
            KeyCode::Char('n') => {
                // Add a new host
                self.add_quick_host().await?;
            }
            KeyCode::Char('e') => {
                // Edit selected host - open config file
                self.edit_config().await?;
            }
            KeyCode::Char('d') => {
                // Delete selected host
                self.delete_selected_host().await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Edit configuration file in external editor
    async fn edit_config(&mut self) -> Result<()> {
        let config_path = Config::config_path();

        // Detect available editor
        let editor = match crate::utils::detect_editor() {
            Some(ed) => ed,
            None => {
                self.status_message = Some("No editor found. Set $EDITOR or install nano/vim/vi.".to_string());
                return Ok(());
            }
        };

        // Pause event polling to avoid consuming keystrokes
        self.events.pause();

        // Small delay to let event loop pause
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Exit TUI mode completely
        self.tui.exit()?;

        // Flush any pending output
        let _ = std::io::stdout().flush();

        // Open editor with proper stdio inheritance
        let status = std::process::Command::new(&editor)
            .arg(&config_path)
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();

        // Small delay to let terminal settle
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Re-enter TUI mode
        self.tui.enter()?;

        // Resume event polling
        self.events.resume();

        // Clear and redraw
        self.tui.clear()?;

        match status {
            Ok(s) if s.success() => {
                // Reload config
                self.config = Config::load().await?;
                self.status_message = Some("Config reloaded".to_string());
                // Reset selection if out of bounds
                let host_count = self.all_hosts().len();
                if self.selected_host_index >= host_count && host_count > 0 {
                    self.selected_host_index = host_count - 1;
                }
            }
            Ok(_) => {
                // :q exits with success code 0, so this handles other exits
                self.config = Config::load().await?;
                self.status_message = Some("Config reloaded".to_string());
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to open editor: {}", e));
            }
        }

        Ok(())
    }

    /// Delete the selected host
    async fn delete_selected_host(&mut self) -> Result<()> {
        if self.all_hosts().is_empty() {
            self.status_message = Some("No host to delete".to_string());
            return Ok(());
        }

        // Find which list the selected host is in
        let mut idx = 0;

        // Check group hosts
        for group in &mut self.config.groups {
            if group.expanded {
                for i in 0..group.hosts.len() {
                    if idx == self.selected_host_index {
                        let removed = group.hosts.remove(i);
                        self.config.save().await?;
                        self.status_message = Some(format!("Deleted host: {}", removed.name));
                        // Adjust selection
                        let total = self.all_hosts().len();
                        if self.selected_host_index >= total && total > 0 {
                            self.selected_host_index = total - 1;
                        }
                        return Ok(());
                    }
                    idx += 1;
                }
            }
        }

        // Check ungrouped hosts
        let ungrouped_idx = self.selected_host_index - idx;
        if ungrouped_idx < self.config.hosts.len() {
            let removed = self.config.hosts.remove(ungrouped_idx);
            self.config.save().await?;
            self.status_message = Some(format!("Deleted host: {}", removed.name));
            // Adjust selection
            let total = self.all_hosts().len();
            if self.selected_host_index >= total && total > 0 {
                self.selected_host_index = total - 1;
            }
        }

        Ok(())
    }

    /// Quick add a new host with minimal prompts
    async fn add_quick_host(&mut self) -> Result<()> {
        use crate::config::HostConfig;

        // Create a new host with sensible defaults
        let username = whoami::username();
        let host_num = self.config.hosts.len() + 1;
        let new_host = HostConfig::new(format!("new-host-{}", host_num), "localhost", username);

        self.config.hosts.push(new_host.clone());
        self.config.save().await?;

        self.status_message = Some(format!(
            "Added host: {} (edit config to customize)",
            new_host.name
        ));

        // Select the new host
        self.selected_host_index = self.all_hosts().len().saturating_sub(1);

        Ok(())
    }

    /// Connect to a host and open a session
    /// Handles proxy chains (jump hosts) with recursive password prompts
    /// Integrates with credential manager for saved passwords
    async fn connect_to_host(&mut self, host: HostConfig) -> Result<()> {
        self.status_message = Some(format!("Connecting to {}...", host.name));

        // Get terminal size
        // Account for: status bar (2 lines), tab bar (2 lines), terminal block borders (2 lines top+bottom, 2 cols left+right)
        let size = self.tui.size()?;
        let cols = size.width.saturating_sub(2) as u32; // Subtract block borders (left + right)
        let rows = size.height.saturating_sub(6) as u32; // Subtract: status bar (2) + tab bar (2) + block borders (2)

        // Clone host name for later use
        let host_name = host.name.clone();
        let host_id = host.id;

        // Resolve the full proxy chain
        let proxy_chain = self.config.resolve_proxy_chain(&host);
        
        // Collect passwords for all hosts in the chain that need password auth
        let mut passwords: std::collections::HashMap<uuid::Uuid, String> = std::collections::HashMap::new();
        let mut hosts_to_save: Vec<uuid::Uuid> = Vec::new();
        
        // Check if any host in the chain needs password auth
        let hosts_needing_password: Vec<_> = proxy_chain
            .iter()
            .filter(|h| matches!(h.auth, crate::config::AuthMethod::Password))
            .collect();
        
        if !hosts_needing_password.is_empty() {
            // Check if any host wants to remember password (for saving) or has saved password (for retrieval)
            let any_wants_remember = hosts_needing_password.iter()
                .any(|h| h.remember_password);
            let has_any_saved = hosts_needing_password.iter()
                .any(|h| h.remember_password && self.credentials.has_saved_password(h.id));
            
            // Need to setup/unlock master password if:
            // - Any host wants to remember password (for future saving), OR
            // - Any host has a saved password (for retrieval)
            if (any_wants_remember || has_any_saved) && !self.credentials.is_unlocked() {
                // Need to unlock master password first
                if !self.credentials.has_master_password() {
                    // First time setup - prompt to create master password
                    self.events.pause();
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    self.tui.exit()?;
                    
                    println!("\n🔐 First time setup: Create a master password to secure your saved credentials.");
                    println!("   This password encrypts all saved connection passwords.\n");
                    
                    print!("Create master password: ");
                    let _ = std::io::stdout().flush();
                    let master_pwd = match rpassword::read_password() {
                        Ok(p) => p,
                        Err(e) => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            self.tui.enter()?;
                            self.events.resume();
                            self.tui.clear()?;
                            self.status_message = Some(format!("Failed to read password: {}", e));
                            return Ok(());
                        }
                    };
                    
                    print!("Confirm master password: ");
                    let _ = std::io::stdout().flush();
                    let confirm_pwd = match rpassword::read_password() {
                        Ok(p) => p,
                        Err(e) => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            self.tui.enter()?;
                            self.events.resume();
                            self.tui.clear()?;
                            self.status_message = Some(format!("Failed to read password: {}", e));
                            return Ok(());
                        }
                    };
                    
                    if master_pwd != confirm_pwd {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        self.tui.enter()?;
                        self.events.resume();
                        self.tui.clear()?;
                        self.status_message = Some("Passwords don't match. Try again.".to_string());
                        return Ok(());
                    }
                    
                    if let Err(e) = self.credentials.setup_master_password(&master_pwd) {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        self.tui.enter()?;
                        self.events.resume();
                        self.tui.clear()?;
                        self.status_message = Some(format!("Failed to setup master password: {}", e));
                        return Ok(());
                    }
                    
                    println!("\n✅ Master password created successfully!\n");
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    self.tui.enter()?;
                    self.events.resume();
                    self.tui.clear()?;
                } else {
                    // Prompt for existing master password
                    self.events.pause();
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    self.tui.exit()?;
                    
                    print!("\n🔐 Master password: ");
                    let _ = std::io::stdout().flush();
                    let master_pwd = match rpassword::read_password() {
                        Ok(p) => p,
                        Err(e) => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            self.tui.enter()?;
                            self.events.resume();
                            self.tui.clear()?;
                            self.status_message = Some(format!("Failed to read password: {}", e));
                            return Ok(());
                        }
                    };
                    
                    match self.credentials.unlock(&master_pwd) {
                        Ok(true) => {
                            println!("✅ Unlocked\n");
                            std::thread::sleep(std::time::Duration::from_millis(200));
                        }
                        Ok(false) => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            self.tui.enter()?;
                            self.events.resume();
                            self.tui.clear()?;
                            self.status_message = Some("Incorrect master password".to_string());
                            return Ok(());
                        }
                        Err(e) => {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            self.tui.enter()?;
                            self.events.resume();
                            self.tui.clear()?;
                            self.status_message = Some(format!("Failed to unlock: {}", e));
                            return Ok(());
                        }
                    }
                    
                    self.tui.enter()?;
                    self.events.resume();
                    self.tui.clear()?;
                }
            }
            
            // Now collect passwords - try saved passwords first
            let mut need_prompt = false;
            for host_config in &hosts_needing_password {
                // Try saved password first if remember_password is enabled
                if host_config.remember_password && self.credentials.is_unlocked() {
                    if let Ok(Some(saved_pwd)) = self.credentials.get_password(host_config.id) {
                        passwords.insert(host_config.id, saved_pwd);
                        continue;
                    }
                }
                need_prompt = true;
            }
            
            if need_prompt {
                // Exit TUI mode to prompt for passwords
                self.events.pause();
                tokio::time::sleep(Duration::from_millis(50)).await;
                self.tui.exit()?;
                
                println!(); // Newline for cleaner output
                
                for host_config in &hosts_needing_password {
                    // Skip if we already have a saved password
                    if passwords.contains_key(&host_config.id) {
                        continue;
                    }
                    
                    // Show context for proxy chain
                    let context = if host_config.id == host.id {
                        "target".to_string()
                    } else {
                        "jump host".to_string()
                    };
                    
                    print!("Password for {}@{} ({}): ", host_config.username, host_config.hostname, context);
                    let _ = std::io::stdout().flush();
                    
                    match rpassword::read_password() {
                        Ok(pwd) => {
                            passwords.insert(host_config.id, pwd);
                            // Mark for potential save if remember_password is enabled
                            if host_config.remember_password {
                                hosts_to_save.push(host_config.id);
                            }
                        }
                        Err(e) => {
                            // Re-enter TUI mode before returning error
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            self.tui.enter()?;
                            self.events.resume();
                            self.tui.clear()?;
                            self.status_message = Some(format!("Failed to read password: {}", e));
                            return Ok(());
                        }
                    }
                }
                
                // Re-enter TUI mode
                std::thread::sleep(std::time::Duration::from_millis(100));
                self.tui.enter()?;
                self.events.resume();
                self.tui.clear()?;
            }
        }

        // Perform connection in blocking context
        let connection_result = tokio::task::spawn_blocking(move || {
            // Connect through the proxy chain
            let mut prev_connection: Option<SshConnection> = None;
            
            for (i, chain_host) in proxy_chain.iter().enumerate() {
                let is_last = i == proxy_chain.len() - 1;
                let password = passwords.get(&chain_host.id).map(|s| s.as_str());
                
                let proxy = if let Some(conn) = prev_connection.take() {
                    ProxyConnection::JumpHost {
                        connection: Box::new(conn),
                    }
                } else {
                    ProxyConnection::Direct
                };
                
                let connection = SshConnection::connect_via_proxy(
                    chain_host.clone(),
                    proxy,
                    password,
                    None, // passphrase
                )?;
                
                if is_last {
                    return Ok((connection, passwords));
                } else {
                    prev_connection = Some(connection);
                }
            }
            
            Err(anyhow::anyhow!("Empty proxy chain"))
        })
        .await?;

        match connection_result {
            Ok((mut connection, passwords_used)) => {
                let connection_id = connection.id;

                // Save passwords for hosts that were newly entered (not from vault) and have remember_password
                for host_to_save_id in &hosts_to_save {
                    if let Some(pwd) = passwords_used.get(host_to_save_id) {
                        // Ensure master password is unlocked (should already be)
                        if self.credentials.is_unlocked() {
                            if let Err(e) = self.credentials.save_password(*host_to_save_id, pwd).await {
                                // Log but don't fail connection
                                tracing::warn!("Failed to save password: {}", e);
                            }
                        }
                    }
                }

                // Open shell channel
                match connection.open_shell(cols, rows) {
                    Ok(channel) => {
                        // Create session for terminal emulation (use same size as PTY)
                        let session_id = self.sessions.create_session(
                            host_id,
                            host_name.clone(),
                            cols as u16,
                            rows as u16,
                        );

                        // Set up channel I/O
                        let (input_tx, input_rx) = mpsc::unbounded_channel::<Vec<u8>>();

                        // Store channel info
                        self.channels.insert(
                            session_id,
                            ActiveChannel {
                                session_id,
                                connection_id,
                                input_tx,
                            },
                        );

                        // Store connection
                        self.connections.add(connection);

                        // Spawn task to handle channel I/O
                        let event_sender = self.events.sender();
                        tokio::task::spawn_blocking(move || {
                            Self::handle_channel_io(channel, session_id, event_sender, input_rx);
                        });

                        // Switch to session view
                        self.active_session = Some(session_id);
                        self.view = View::Session;
                        self.status_message = Some(format!("Connected to {}", host_name));
                        
                        // Store password for SFTP reuse (if password was used)
                        if let Some(pwd) = passwords_used.get(&host_id) {
                            self.session_passwords.insert(host_id, pwd.clone());
                        }
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to open shell: {}", e));
                    }
                }
            }
            Err(e) => {
                // Connection failed - if we used saved passwords, clear them
                for host_to_save_id in &hosts_to_save {
                    if self.credentials.has_saved_password(*host_to_save_id) {
                        if let Err(del_err) = self.credentials.delete_password(*host_to_save_id).await {
                            tracing::warn!("Failed to delete invalid password: {}", del_err);
                        }
                    }
                }
                self.status_message = Some(format!("Connection failed: {}", e));
            }
        }

        Ok(())
    }

    /// Open SFTP session for a host, creating a new SSH connection for SFTP
    /// We create a separate connection because ssh2 sessions can't safely
    /// share between blocking SFTP and non-blocking shell I/O
    async fn open_sftp_for_host(&mut self, host: HostConfig) -> Result<()> {
        let host_id = host.id;
        let host_name = host.name.clone();
        
        // Check if we already have an SFTP session for this host
        if self.sftp_sessions.get_by_host(host_id).is_some() {
            self.active_sftp_host = Some(host_id);
            self.push_view(View::Sftp);
            
            // Initialize file browser if needed
            if self.file_browser.is_none() {
                let mut browser = FileBrowser::new();
                browser.left.load_local().await?;
                self.file_browser = Some(browser);
            }
            
            // Reload remote pane
            if let Some(browser) = &mut self.file_browser {
                if let Some(sftp_session) = self.sftp_sessions.get_by_host(host_id) {
                    browser.right.path = sftp_session.cwd.clone();
                    let _ = browser.right.load_remote(sftp_session);
                }
            }
            
            return Ok(());
        }
        
        // Get password: first check session passwords (from current session), then credential manager
        let password = self.session_passwords.get(&host_id).cloned()
            .or_else(|| {
                if self.credentials.is_unlocked() {
                    self.credentials.get_password(host_id).ok().flatten()
                } else {
                    None
                }
            });
        
        // Check if we have a password or if the host uses key/agent authentication
        let has_key_auth = matches!(
            host.auth,
            crate::config::AuthMethod::KeyFile { .. } | 
            crate::config::AuthMethod::Agent | 
            crate::config::AuthMethod::Certificate { .. }
        );
        if password.is_none() && !has_key_auth {
            self.status_message = Some(format!(
                "No password available for SFTP. Reconnect to {} first.",
                host_name
            ));
            return Ok(());
        }
        
        // Create a new connection for SFTP (separate from shell connection)
        // Must use proxy chain just like connect_to_host does
        let pwd_source = if self.session_passwords.contains_key(&host_id) {
            "session"
        } else if password.is_some() {
            "credential manager"
        } else {
            "none"
        };
        let has_pwd = password.is_some();
        
        // Resolve the full proxy chain (same as connect_to_host)
        let proxy_chain = self.config.resolve_proxy_chain(&host);
        
        // Collect passwords for all hosts in the chain
        let mut passwords: std::collections::HashMap<uuid::Uuid, String> = std::collections::HashMap::new();
        
        // Get passwords for all hosts in the chain
        for chain_host in &proxy_chain {
            // First check session passwords
            if let Some(pwd) = self.session_passwords.get(&chain_host.id) {
                passwords.insert(chain_host.id, pwd.clone());
            } else if self.credentials.is_unlocked() {
                // Try credential manager
                if let Ok(Some(pwd)) = self.credentials.get_password(chain_host.id) {
                    passwords.insert(chain_host.id, pwd);
                }
            }
        }
        
        // For the target host, also try the password variable we already retrieved
        if let Some(pwd) = password.clone() {
            passwords.entry(host_id).or_insert(pwd);
        }
        
        let sftp_result = tokio::task::spawn_blocking(move || {
            // Connect through the proxy chain (same logic as connect_to_host)
            let mut prev_connection: Option<SshConnection> = None;
            
            for (i, chain_host) in proxy_chain.iter().enumerate() {
                let is_last = i == proxy_chain.len() - 1;
                let password = passwords.get(&chain_host.id).map(|s| s.as_str());
                
                let proxy = if let Some(conn) = prev_connection.take() {
                    ProxyConnection::JumpHost {
                        connection: Box::new(conn),
                    }
                } else {
                    ProxyConnection::Direct
                };
                
                let connection = SshConnection::connect_via_proxy(
                    chain_host.clone(),
                    proxy,
                    password,
                    None, // passphrase
                )?;
                
                if is_last {
                    // Open SFTP on the final connection (target host)
                    let sftp = connection.open_sftp()?;
                    return Ok::<_, anyhow::Error>((connection, sftp));
                } else {
                    prev_connection = Some(connection);
                }
            }
            
            Err(anyhow::anyhow!("Empty proxy chain"))
        }).await?;
        
        match sftp_result {
            Ok((connection, sftp)) => {
                let conn_id = connection.id;
                
                // Store connection
                self.connections.add(connection);
                
                // Create SFTP session with username for home directory
                let username = &host.username;
                let sftp_session = SftpSession::new(sftp, host_id, conn_id, username)?;
                let session_cwd = sftp_session.cwd.clone();
                self.sftp_sessions.add(sftp_session);
                self.active_sftp_host = Some(host_id);
                
                // Initialize file browser
                let mut browser = if let Some(b) = self.file_browser.take() {
                    b
                } else {
                    FileBrowser::new()
                };
                
                browser.left.load_local().await?;
                browser.right.path = session_cwd;
                
                if let Some(sftp_session) = self.sftp_sessions.get_by_host(host_id) {
                    let _ = browser.right.load_remote(sftp_session);
                }
                
                self.file_browser = Some(browser);
                self.push_view(View::Sftp);
                self.status_message = Some(format!("SFTP connected to {}", host_name));
            }
            Err(e) => {
                self.status_message = Some(format!("SFTP failed (pwd from {}, has_pwd={}): {}", pwd_source, has_pwd, e));
            }
        }
        
        Ok(())
    }

    /// Handle channel I/O in a blocking context
    fn handle_channel_io(
        mut channel: ssh2::Channel,
        session_id: Uuid,
        event_sender: mpsc::UnboundedSender<AppEvent>,
        mut input_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    ) {
        use std::io::Write;

        // Note: ssh2 blocking is controlled at Session level
        // We'll use short reads with timeout

        let mut buf = [0u8; 4096];

        loop {
            // Check if channel is closed
            if channel.eof() {
                let _ = event_sender.send(AppEvent::SshDisconnected {
                    session_id,
                    reason: "Channel closed".to_string(),
                });
                break;
            }

            // Read available data
            match channel.read(&mut buf) {
                Ok(0) => {
                    // No data available, check for input
                }
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    if event_sender
                        .send(AppEvent::SshData { session_id, data })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available
                }
                Err(e) => {
                    let _ = event_sender.send(AppEvent::SshDisconnected {
                        session_id,
                        reason: format!("Read error: {}", e),
                    });
                    break;
                }
            }

            // Check for input to send
            match input_rx.try_recv() {
                Ok(data) => {
                    if let Err(e) = channel.write_all(&data) {
                        let _ = event_sender.send(AppEvent::SshDisconnected {
                            session_id,
                            reason: format!("Write error: {}", e),
                        });
                        break;
                    }
                    let _ = channel.flush();
                }
                Err(mpsc::error::TryRecvError::Empty) => {}
                Err(mpsc::error::TryRecvError::Disconnected) => break,
            }

            // Small sleep to prevent busy loop
            std::thread::sleep(Duration::from_millis(10));
        }

        let _ = channel.close();
    }

    async fn handle_session_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        // Check for escape back to connections
        if key.code == KeyCode::Esc && key.modifiers.contains(KeyModifiers::SHIFT) {
            self.view = View::Connections;
            return Ok(());
        }

        // Check for Shift+F to open SFTP for current session's host
        if key.code == KeyCode::Char('F') && key.modifiers.contains(KeyModifiers::SHIFT) {
            // Get the host ID from the current session
            if let Some(session_id) = self.active_session {
                if let Some(session) = self.sessions.get(session_id) {
                    let host_id = session.host_id;
                    // Find the host config
                    if let Some(host) = self.all_hosts().iter().find(|h| h.id == host_id).cloned() {
                        self.open_sftp_for_host(host.clone()).await?;
                        return Ok(());
                    }
                }
            }
            self.status_message = Some("No active session for SFTP".to_string());
            return Ok(());
        }

        // Forward key to SSH session
        if let Some(session_id) = self.active_session {
            if let Some(channel) = self.channels.get(&session_id) {
                let data = self.key_to_bytes(&key);
                if !data.is_empty() {
                    let _ = channel.input_tx.send(data);
                }
            }
        }

        Ok(())
    }

    async fn handle_sftp_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        // Handle SFTP-specific keys
        match key.code {
            // Navigation keys
            KeyCode::Esc => {
                self.pop_view();
                // Clear active SFTP host only if not returning to SFTP
                if self.view != View::Sftp {
                    self.active_sftp_host = None;
                }
            }
            KeyCode::Char('?') => self.view = View::Help,
            
            // Pane switching
            KeyCode::Tab => {
                if let Some(browser) = &mut self.file_browser {
                    browser.switch_pane();
                }
            }
            
            // Cursor movement
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().cursor_up();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().cursor_down();
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().cursor_top();
                }
            }
            KeyCode::End | KeyCode::Char('G') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().cursor_bottom();
                }
            }
            KeyCode::PageUp => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().page_up(10);
                }
            }
            KeyCode::PageDown => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().page_down(10);
                }
            }
            
            // Selection
            KeyCode::Char(' ') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().toggle_selection();
                    browser.active_pane_mut().cursor_down();
                }
            }
            
            // Enter directory or trigger transfer
            KeyCode::Enter => {
                self.handle_sftp_enter().await?;
            }
            
            // Backspace goes up a directory
            KeyCode::Backspace => {
                self.handle_sftp_go_parent().await?;
            }
            
            // Copy to other pane (F5 or 'c')
            KeyCode::F(5) | KeyCode::Char('c') => {
                self.handle_sftp_copy().await?;
            }
            
            // Move to other pane (F6 or 'm')
            KeyCode::F(6) | KeyCode::Char('m') => {
                self.handle_sftp_move().await?;
            }
            
            // Delete (F8 or 'd') - local only
            KeyCode::F(8) | KeyCode::Char('d') => {
                self.handle_sftp_delete().await?;
            }
            
            // New directory (n)
            KeyCode::Char('n') => {
                self.status_message = Some("New directory: not yet implemented".to_string());
            }
            
            // Rename (r)
            KeyCode::Char('r') => {
                self.status_message = Some("Rename: not yet implemented".to_string());
            }
            
            // Toggle hidden files (h)
            KeyCode::Char('h') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().toggle_hidden();
                }
            }
            
            // Sort cycling (s)
            KeyCode::Char('s') => {
                if let Some(browser) = &mut self.file_browser {
                    browser.active_pane_mut().cycle_sort();
                }
            }
            
            // Refresh (F2)
            KeyCode::F(2) => {
                self.handle_sftp_refresh().await?;
            }
            
            _ => {}
        }
        Ok(())
    }

    /// Handle Enter key in SFTP view - navigate into directory or start transfer
    async fn handle_sftp_enter(&mut self) -> Result<()> {
        let entry_info = if let Some(browser) = &self.file_browser {
            browser.active_pane().current_entry().map(|e| (e.is_dir, e.path.clone()))
        } else {
            None
        };

        if let Some((is_dir, path)) = entry_info {
            if is_dir {
                // Navigate into directory
                if let Some(browser) = &mut self.file_browser {
                    let is_remote = browser.active_pane().is_remote;
                    browser.active_pane_mut().path = path;
                    browser.active_pane_mut().cursor = 0;
                    
                    if is_remote {
                        // Reload remote pane
                        if let Some(host_id) = self.active_sftp_host {
                            if let Some(sftp_session) = self.sftp_sessions.get_by_host(host_id) {
                                let path = browser.active_pane().path.clone();
                                if let Err(e) = browser.active_pane_mut().load_remote(sftp_session) {
                                    self.status_message = Some(format!("Failed to load {}: {}", path.display(), e));
                                }
                            }
                        }
                    } else {
                        // Reload local pane
                        if let Err(e) = browser.active_pane_mut().load_local().await {
                            self.status_message = Some(format!("Failed to load directory: {}", e));
                        }
                    }
                }
            } else {
                // File selected - initiate transfer
                self.handle_sftp_copy().await?;
            }
        }
        Ok(())
    }

    /// Go to parent directory in SFTP view
    async fn handle_sftp_go_parent(&mut self) -> Result<()> {
        let changed = if let Some(browser) = &mut self.file_browser {
            browser.active_pane_mut().go_parent()
        } else {
            false
        };
        
        if changed {
            self.handle_sftp_refresh().await?;
        }
        Ok(())
    }

    /// Refresh current directory in SFTP view
    async fn handle_sftp_refresh(&mut self) -> Result<()> {
        if let Some(browser) = &mut self.file_browser {
            let is_remote = browser.active_pane().is_remote;
            
            if is_remote {
                if let Some(host_id) = self.active_sftp_host {
                    if let Some(sftp_session) = self.sftp_sessions.get_by_host(host_id) {
                        if let Err(e) = browser.active_pane_mut().load_remote(sftp_session) {
                            self.status_message = Some(format!("Refresh failed: {}", e));
                        }
                    }
                }
            } else {
                if let Err(e) = browser.active_pane_mut().load_local().await {
                    self.status_message = Some(format!("Refresh failed: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Handle copy operation in SFTP view
    async fn handle_sftp_copy(&mut self) -> Result<()> {
        // Get selected files from active pane
        let (source_files, is_upload) = if let Some(browser) = &self.file_browser {
            let active = browser.active_pane();
            let selected = active.selected_entries();
            
            // If no selections, use current entry
            let files: Vec<_> = if selected.is_empty() {
                active.current_entry()
                    .filter(|e| e.name != "..")
                    .into_iter()
                    .collect()
            } else {
                selected.into_iter().filter(|e| e.name != "..").collect()
            };
            
            let is_upload = !active.is_remote; // Uploading if source is local
            (files.iter().map(|f| (f.path.clone(), f.size)).collect::<Vec<_>>(), is_upload)
        } else {
            return Ok(());
        };
        
        if source_files.is_empty() {
            self.status_message = Some("No files selected".to_string());
            return Ok(());
        }
        
        // Get SFTP host and destination path
        let host_id = match self.active_sftp_host {
            Some(id) => id,
            None => {
                self.status_message = Some("No active SFTP connection".to_string());
                return Ok(());
            }
        };
        
        let dest_path = self.file_browser.as_ref()
            .map(|b| b.inactive_pane().path.clone())
            .unwrap_or_default();
        
        let direction = if is_upload {
            crate::sftp::TransferDirection::Upload
        } else {
            crate::sftp::TransferDirection::Download
        };
        
        // Get SFTP session for transfer
        let sftp_session = match self.sftp_sessions.get_by_host(host_id) {
            Some(s) => s,
            None => {
                self.status_message = Some("SFTP session not found".to_string());
                return Ok(());
            }
        };
        
        let file_count = source_files.len();
        
        for (source, size) in source_files {
            let filename = source.file_name().map(|n| n.to_os_string()).unwrap_or_default();
            let dest = dest_path.join(&filename);
            
            // Execute transfer immediately (synchronous for now, proper async later)
            let result = if is_upload {
                // Upload: read local file, write to remote
                self.execute_upload(&source, &dest, sftp_session)
            } else {
                // Download: read from remote, write to local
                self.execute_download(&source, &dest, sftp_session)
            };
            
            match result {
                Ok(_) => {
                    self.status_message = Some(format!("Transferred: {}", filename.to_string_lossy()));
                }
                Err(e) => {
                    self.status_message = Some(format!("Transfer failed: {}", e));
                }
            }
        }
        
        // Refresh destination pane
        if let Some(browser) = &mut self.file_browser {
            if is_upload {
                // Refresh remote pane after upload
                if let Some(sftp_session) = self.sftp_sessions.get_by_host(host_id) {
                    let _ = browser.right.load_remote(sftp_session);
                }
            } else {
                // Refresh local pane after download
                let _ = browser.left.load_local().await;
            }
        }
        
        self.status_message = Some(format!("Transferred {} file(s)", file_count));
        Ok(())
    }

    /// Execute upload from local to remote
    fn execute_upload(&self, source: &std::path::Path, dest: &std::path::Path, sftp_session: &SftpSession) -> Result<()> {
        use std::io::{Read, Write};
        
        // Read local file
        let mut local_file = std::fs::File::open(source)?;
        let mut buffer = Vec::new();
        local_file.read_to_end(&mut buffer)?;
        
        // Write to remote
        let mut remote_file = sftp_session.create(dest)?;
        remote_file.write_all(&buffer)?;
        
        Ok(())
    }

    /// Execute download from remote to local
    fn execute_download(&self, source: &std::path::Path, dest: &std::path::Path, sftp_session: &SftpSession) -> Result<()> {
        use std::io::{Read, Write};
        
        // Read remote file
        let mut remote_file = sftp_session.open_read(source)?;
        let mut buffer = Vec::new();
        remote_file.read_to_end(&mut buffer)?;
        
        // Write to local
        let mut local_file = std::fs::File::create(dest)?;
        local_file.write_all(&buffer)?;
        
        Ok(())
    }

    /// Handle move operation in SFTP view
    async fn handle_sftp_move(&mut self) -> Result<()> {
        // For now, same as copy but we would mark for deletion after
        self.status_message = Some("Move: not yet implemented (use copy + delete)".to_string());
        Ok(())
    }

    /// Handle delete operation in SFTP view - LOCAL ONLY for safety
    async fn handle_sftp_delete(&mut self) -> Result<()> {
        if let Some(browser) = &self.file_browser {
            if browser.active_pane().is_remote {
                self.status_message = Some("Delete disabled on remote for safety".to_string());
                return Ok(());
            }
        }
        
        // Get selected files from active pane (local only)
        let files_to_delete: Vec<_> = if let Some(browser) = &self.file_browser {
            let active = browser.active_pane();
            let selected = active.selected_entries();
            
            if selected.is_empty() {
                active.current_entry()
                    .filter(|e| e.name != "..")
                    .into_iter()
                    .map(|e| e.path.clone())
                    .collect()
            } else {
                selected.into_iter()
                    .filter(|e| e.name != "..")
                    .map(|e| e.path.clone())
                    .collect()
            }
        } else {
            return Ok(());
        };
        
        if files_to_delete.is_empty() {
            self.status_message = Some("No files selected for deletion".to_string());
            return Ok(());
        }
        
        // TODO: Add confirmation prompt
        self.status_message = Some(format!("Delete {} file(s): confirmation not yet implemented", files_to_delete.len()));
        Ok(())
    }

    async fn handle_tunnels_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => self.view = View::Connections,
            KeyCode::Char('?') => self.view = View::Help,
            _ => {}
        }
        Ok(())
    }

    async fn handle_keys_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => self.view = View::Connections,
            KeyCode::Char('?') => self.view = View::Help,
            _ => {}
        }
        Ok(())
    }

    async fn handle_settings_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => self.view = View::Connections,
            _ => {}
        }
        Ok(())
    }

    async fn handle_help_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.view = View::Connections;
            }
            _ => {}
        }
        Ok(())
    }

    /// Convert key event to bytes for SSH transmission
    fn key_to_bytes(&self, key: &crossterm::event::KeyEvent) -> Vec<u8> {
        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    // Ctrl+A = 0x01, Ctrl+B = 0x02, etc.
                    let ctrl_code = (c as u8).saturating_sub(b'a' - 1);
                    vec![ctrl_code]
                } else {
                    c.to_string().into_bytes()
                }
            }
            KeyCode::Enter => vec![b'\r'],
            KeyCode::Backspace => vec![0x7f],
            KeyCode::Tab => vec![b'\t'],
            KeyCode::Esc => vec![0x1b],
            KeyCode::Up => vec![0x1b, b'[', b'A'],
            KeyCode::Down => vec![0x1b, b'[', b'B'],
            KeyCode::Right => vec![0x1b, b'[', b'C'],
            KeyCode::Left => vec![0x1b, b'[', b'D'],
            KeyCode::Home => vec![0x1b, b'[', b'H'],
            KeyCode::End => vec![0x1b, b'[', b'F'],
            KeyCode::PageUp => vec![0x1b, b'[', b'5', b'~'],
            KeyCode::PageDown => vec![0x1b, b'[', b'6', b'~'],
            KeyCode::Delete => vec![0x1b, b'[', b'3', b'~'],
            KeyCode::Insert => vec![0x1b, b'[', b'2', b'~'],
            KeyCode::F(n) => match n {
                1 => vec![0x1b, b'O', b'P'],
                2 => vec![0x1b, b'O', b'Q'],
                3 => vec![0x1b, b'O', b'R'],
                4 => vec![0x1b, b'O', b'S'],
                5 => vec![0x1b, b'[', b'1', b'5', b'~'],
                6 => vec![0x1b, b'[', b'1', b'7', b'~'],
                7 => vec![0x1b, b'[', b'1', b'8', b'~'],
                8 => vec![0x1b, b'[', b'1', b'9', b'~'],
                9 => vec![0x1b, b'[', b'2', b'0', b'~'],
                10 => vec![0x1b, b'[', b'2', b'1', b'~'],
                11 => vec![0x1b, b'[', b'2', b'3', b'~'],
                12 => vec![0x1b, b'[', b'2', b'4', b'~'],
                _ => vec![],
            },
            _ => vec![],
        }
    }
}
