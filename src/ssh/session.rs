//! SSH session management

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use uuid::Uuid;
use vt100::Parser;

/// Session status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Connecting,
    Connected,
    Disconnected,
}

/// Interactive SSH session with terminal emulation
pub struct Session {
    /// Session ID
    pub id: Uuid,
    /// Host ID this session belongs to
    pub host_id: Uuid,
    /// Session name (for display)
    pub name: String,
    /// VT100 terminal parser
    vt: Parser,
    /// Scrollback buffer
    scrollback: VecDeque<String>,
    /// Maximum scrollback lines
    scrollback_limit: usize,
    /// Current scroll position (0 = bottom)
    pub scroll_offset: usize,
    /// Session status
    pub status: SessionStatus,
    /// Creation time
    pub created_at: DateTime<Utc>,
    /// Terminal dimensions
    pub cols: u16,
    pub rows: u16,
}

impl Session {
    /// Create a new session
    pub fn new(host_id: Uuid, name: String, cols: u16, rows: u16) -> Self {
        Self {
            id: Uuid::new_v4(),
            host_id,
            name,
            vt: Parser::new(rows, cols, 1000), // 1000 lines of scrollback
            scrollback: VecDeque::new(),
            scrollback_limit: 10000,
            scroll_offset: 0,
            status: SessionStatus::Connecting,
            created_at: Utc::now(),
            cols,
            rows,
        }
    }

    /// Process received data through VT100 parser
    pub fn process_data(&mut self, data: &[u8]) {
        // Auto-scroll to bottom when new data arrives
        self.vt.set_scrollback(0);
        self.scroll_offset = 0;
        // Process through VT100
        self.vt.process(data);
    }

    /// Get current screen content
    pub fn screen(&self) -> &vt100::Screen {
        self.vt.screen()
    }

    /// Get screen content as strings
    /// When scrolled up, vt100 automatically shows scrollback content via cell()
    pub fn screen_lines(&self) -> Vec<String> {
        let screen = self.vt.screen();
        let mut lines = Vec::new();
        
        for row in 0..screen.size().0 {
            let mut line = String::new();
            for col in 0..screen.size().1 {
                if let Some(cell) = screen.cell(row, col) {
                    line.push(cell.contents().chars().next().unwrap_or(' '));
                } else {
                    line.push(' ');
                }
            }
            lines.push(line.trim_end().to_string());
        }
        
        lines
    }

    /// Resize terminal
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.vt.set_size(rows, cols);
    }

    /// Scroll up (view older content)
    pub fn scroll_up(&mut self, lines: usize) {
        // vt100's scrollback() returns current scroll position
        // Use set_scrollback() to scroll into history
        let current = self.vt.screen().scrollback();
        self.vt.set_scrollback(current + lines);
        self.scroll_offset = self.vt.screen().scrollback();
    }

    /// Scroll down (view newer content)
    pub fn scroll_down(&mut self, lines: usize) {
        let current = self.vt.screen().scrollback();
        self.vt.set_scrollback(current.saturating_sub(lines));
        self.scroll_offset = self.vt.screen().scrollback();
    }

    /// Scroll to bottom
    pub fn scroll_to_bottom(&mut self) {
        self.vt.set_scrollback(0);
        self.scroll_offset = 0;
    }

    /// Get cursor position
    pub fn cursor_position(&self) -> (u16, u16) {
        let screen = self.vt.screen();
        screen.cursor_position()
    }

    /// Check if cursor is visible
    pub fn cursor_visible(&self) -> bool {
        !self.vt.screen().hide_cursor()
    }
}

/// Session manager for multiple sessions
pub struct SessionManager {
    sessions: HashMap<Uuid, Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Create a new session
    pub fn create_session(&mut self, host_id: Uuid, name: String, cols: u16, rows: u16) -> Uuid {
        let session = Session::new(host_id, name, cols, rows);
        let id = session.id;
        self.sessions.insert(id, session);
        id
    }

    /// Get a session by ID
    pub fn get(&self, id: Uuid) -> Option<&Session> {
        self.sessions.get(&id)
    }

    /// Get a mutable session by ID
    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut Session> {
        self.sessions.get_mut(&id)
    }

    /// Remove a session
    pub fn remove(&mut self, id: Uuid) -> Option<Session> {
        self.sessions.remove(&id)
    }

    /// List all sessions
    pub fn list(&self) -> Vec<&Session> {
        self.sessions.values().collect()
    }

    /// Process data for a session
    pub async fn process_data(&mut self, session_id: Uuid, data: &[u8]) -> Result<()> {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.process_data(data);
        }
        Ok(())
    }

    /// Resize a session
    pub async fn resize_session(&mut self, session_id: Uuid, cols: u16, rows: u16) -> Result<()> {
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.resize(cols, rows);
        }
        Ok(())
    }

    /// Send data placeholder (actual sending happens through channel)
    pub async fn send_data(&mut self, _session_id: Uuid, _data: &[u8]) -> Result<()> {
        // Data sending is handled by the SSH channel in the main loop
        Ok(())
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}
