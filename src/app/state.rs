//! Application state management

use crate::config::Config;
use crate::ssh::SessionManager;
use crate::tui::{Tui, Theme};
use super::{AppEvent, EventHandler};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use std::time::Duration;
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
}

/// Main application struct
pub struct App {
    /// Current view
    pub view: View,
    /// Application state
    pub state: AppState,
    /// Configuration
    pub config: Config,
    /// SSH session manager
    pub sessions: SessionManager,
    /// Active session ID (if in session view)
    pub active_session: Option<Uuid>,
    /// Status message
    pub status_message: Option<String>,
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
        let events = EventHandler::new(Duration::from_millis(100));

        Ok(Self {
            view: View::default(),
            state: AppState::default(),
            config,
            sessions: SessionManager::new(),
            active_session: None,
            status_message: None,
            tui,
            events,
            theme,
        })
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
                self.status_message = Some(format!("Session disconnected: {}", reason));
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
        match key.code {
            KeyCode::Char('q') => self.state = AppState::Quit,
            KeyCode::Char('?') => self.view = View::Help,
            KeyCode::Char('s') => self.view = View::Settings,
            KeyCode::Char('k') => self.view = View::Keys,
            KeyCode::Char('t') => self.view = View::Tunnels,
            KeyCode::Char('f') => self.view = View::Sftp,
            KeyCode::Enter => {
                // TODO: Connect to selected host
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_session_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if let Some(session_id) = self.active_session {
            // Forward key to SSH session
            let data = self.key_to_bytes(&key);
            if !data.is_empty() {
                self.sessions.send_data(session_id, &data).await?;
            }
        }

        // Escape to connections view
        if key.code == KeyCode::Esc && key.modifiers.contains(KeyModifiers::SHIFT) {
            self.view = View::Connections;
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
