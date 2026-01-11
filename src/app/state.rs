//! Application state management

use crate::config::{Config, HostConfig};
use crate::ssh::{SessionManager, SshConnection, ConnectionPool};
use crate::tui::{Tui, Theme};
use super::{AppEvent, EventHandler};
use anyhow::{anyhow, Result};
use crossterm::event::{KeyCode, KeyModifiers};
use std::collections::HashMap;
use std::io::Read;
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
    pub screen_lines: Vec<String>,
    pub cursor_position: (u16, u16),
    pub cursor_visible: bool,
}

/// Render state snapshot (avoids borrow conflicts in draw callback)
#[derive(Clone)]
pub struct RenderState {
    pub view: View,
    pub theme: Theme,
    pub config: Config,
    pub sessions: Vec<SessionInfo>,
    pub active_session: Option<Uuid>,
    pub status_message: Option<String>,
    pub selected_host_index: usize,
    pub host_count: usize,
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
}

impl App {
    /// Create a new application instance
    pub async fn new() -> Result<Self> {
        let config = Config::load().await?;
        let theme = Theme::default();
        let tui = Tui::new()?;
        let events = EventHandler::new(Duration::from_millis(50));

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

    /// Run the main application loop
    pub async fn run(&mut self) -> Result<()> {
        self.tui.enter()?;
        self.events.start();

        while self.state != AppState::Quit {
            // Create render state to avoid borrow conflict
            let render_state = RenderState {
                view: self.view,
                theme: self.theme.clone(),
                config: self.config.clone(),
                sessions: self.sessions.list().iter().map(|s| SessionInfo {
                    id: s.id,
                    name: s.name.clone(),
                    screen_lines: s.screen_lines(),
                    cursor_position: s.cursor_position(),
                    cursor_visible: s.cursor_visible(),
                }).collect(),
                active_session: self.active_session,
                status_message: self.status_message.clone(),
                selected_host_index: self.selected_host_index,
                host_count: self.all_hosts().len(),
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
                // Global keybindings
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('c') | KeyCode::Char('q') => {
                            self.state = AppState::Quit;
                        }
                        _ => {}
                    }
                } else {
                    self.handle_key(key).await?;
                }
            }
            AppEvent::Resize(w, h) => {
                // Handle terminal resize
                if let Some(session_id) = self.active_session {
                    self.sessions.resize_session(session_id, w, h).await?;
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
            _ => {}
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

    async fn handle_connections_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        let host_count = self.all_hosts().len();
        
        match key.code {
            KeyCode::Char('q') => self.state = AppState::Quit,
            KeyCode::Char('?') => self.view = View::Help,
            KeyCode::Char('s') => self.view = View::Settings,
            KeyCode::Char('K') => self.view = View::Keys, // Shift+K for Keys view
            KeyCode::Char('t') => self.view = View::Tunnels,
            KeyCode::Char('f') => self.view = View::Sftp,
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
        
        // Get editor from environment or use default
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        
        // Exit TUI temporarily
        self.tui.exit()?;
        
        // Open editor
        let status = std::process::Command::new(&editor)
            .arg(&config_path)
            .status();
        
        // Re-enter TUI
        self.tui.enter()?;
        
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
                self.status_message = Some("Editor exited with error".to_string());
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
        let new_host = HostConfig::new(
            format!("new-host-{}", host_num),
            "localhost",
            username,
        );
        
        self.config.hosts.push(new_host.clone());
        self.config.save().await?;
        
        self.status_message = Some(format!("Added host: {} (edit config to customize)", new_host.name));
        
        // Select the new host
        self.selected_host_index = self.all_hosts().len().saturating_sub(1);
        
        Ok(())
    }

    /// Connect to a host and open a session
    async fn connect_to_host(&mut self, host: HostConfig) -> Result<()> {
        self.status_message = Some(format!("Connecting to {}...", host.name));
        
        // Get terminal size
        let size = self.tui.size()?;
        let cols = size.width as u32;
        let rows = size.height.saturating_sub(2) as u32; // Leave room for status bar
        
        // Clone host name for later use
        let host_name = host.name.clone();
        let host_id = host.id;
        
        // Perform connection in blocking context
        let connection_result = tokio::task::spawn_blocking(move || {
            // TODO: For password auth, we'd need to prompt the user
            // For now, try agent auth first
            SshConnection::connect(host, None, None)
        }).await?;
        
        match connection_result {
            Ok(mut connection) => {
                let connection_id = connection.id;
                
                // Open shell channel
                match connection.open_shell(cols, rows) {
                    Ok(channel) => {
                        // Create session for terminal emulation
                        let session_id = self.sessions.create_session(
                            host_id,
                            host_name.clone(),
                            size.width,
                            size.height.saturating_sub(2),
                        );
                        
                        // Set up channel I/O
                        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
                        
                        // Store channel info
                        self.channels.insert(session_id, ActiveChannel {
                            session_id,
                            connection_id,
                            input_tx,
                        });
                        
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
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to open shell: {}", e));
                    }
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Connection failed: {}", e));
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
                    if event_sender.send(AppEvent::SshData { session_id, data }).is_err() {
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
        match key.code {
            KeyCode::Esc => self.view = View::Connections,
            KeyCode::Char('?') => self.view = View::Help,
            _ => {}
        }
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
            KeyCode::F(n) => {
                match n {
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
                }
            }
            _ => vec![],
        }
    }
}
