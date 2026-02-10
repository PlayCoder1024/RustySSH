//! SSH session management

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;
use vt100::Parser;

/// Session status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Connecting,
    Connected,
    Disconnected,
}

/// Text selection in terminal (start and end positions)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSelection {
    /// Start position (row, col)
    pub start: (u16, u16),
    /// End position (row, col)
    pub end: (u16, u16),
}

impl TextSelection {
    /// Get normalized selection (start before end)
    pub fn normalized(&self) -> ((u16, u16), (u16, u16)) {
        if self.start.0 < self.end.0 || (self.start.0 == self.end.0 && self.start.1 <= self.end.1) {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }

    /// Check if a cell is within the selection
    pub fn contains(&self, row: u16, col: u16) -> bool {
        let ((start_row, start_col), (end_row, end_col)) = self.normalized();

        if row < start_row || row > end_row {
            return false;
        }

        if start_row == end_row {
            // Single line selection
            col >= start_col && col <= end_col
        } else if row == start_row {
            // First line of multi-line selection
            col >= start_col
        } else if row == end_row {
            // Last line of multi-line selection
            col <= end_col
        } else {
            // Middle lines are fully selected
            true
        }
    }
}

/// Interactive SSH session with terminal emulation
#[allow(unused)]
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
    /// Current text selection
    pub selection: Option<TextSelection>,
    /// Whether user is currently selecting (mouse drag in progress)
    pub is_selecting: bool,
    /// Progress of current operation (0.0 to 1.0), or None if idle
    pub progress: Option<f32>,
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
            selection: None,
            is_selecting: false,
            progress: None,
        }
    }

    /// Process received data through VT100 parser
    pub fn process_data(&mut self, data: &[u8]) {
        // Auto-scroll to bottom when new data arrives
        self.vt.screen_mut().set_scrollback(0);
        self.scroll_offset = 0;

        // Parse OSC 9;4 progress sequences before passing to VT100
        // Format: ESC ] 9;4;STATE;VALUE BEL  or  ESC ] 9;4;STATE;VALUE ST
        // STATE: 0=remove, 1=set, 2=error, 3=indeterminate
        // VALUE: 0-100 (percentage)
        self.parse_osc_progress(data);

        // Process through VT100
        self.vt.process(data);
    }

    /// Parse OSC 9;4 progress sequences from terminal data
    fn parse_osc_progress(&mut self, data: &[u8]) {
        // Look for ESC ] 9;4; pattern
        let mut i = 0;
        while i < data.len() {
            // Check for ESC ] (0x1B 0x5D)
            if data[i] == 0x1B && i + 1 < data.len() && data[i + 1] == 0x5D {
                // Found OSC start, look for "9;4;"
                let osc_start = i + 2;
                if let Some(osc_body) = data.get(osc_start..) {
                    if osc_body.starts_with(b"9;4;") {
                        // Parse state and optional value
                        let params_start = osc_start + 4; // skip "9;4;"
                                                          // Find the terminator: BEL (0x07) or ST (ESC \)
                        let mut end = params_start;
                        while end < data.len() {
                            if data[end] == 0x07 {
                                break;
                            }
                            if data[end] == 0x1B && end + 1 < data.len() && data[end + 1] == b'\\' {
                                break;
                            }
                            end += 1;
                        }
                        if end > params_start {
                            let params = &data[params_start..end];
                            if let Ok(params_str) = std::str::from_utf8(params) {
                                let parts: Vec<&str> = params_str.splitn(2, ';').collect();
                                let state: u8 = parts[0].parse().unwrap_or(0);
                                let value: f32 = parts
                                    .get(1)
                                    .and_then(|v| v.parse::<f32>().ok())
                                    .unwrap_or(0.0);

                                match state {
                                    0 => self.progress = None,                // Remove progress
                                    1 => self.progress = Some(value / 100.0), // Normal progress
                                    2 => self.progress = Some(value / 100.0), // Error (still show)
                                    3 => self.progress = Some(-1.0),          // Indeterminate
                                    _ => {}
                                }
                            }
                        }
                        i = end;
                        continue;
                    }
                }
            }
            i += 1;
        }
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
        self.vt.screen_mut().set_size(rows, cols);
    }

    /// Scroll up (view older content)
    pub fn scroll_up(&mut self, lines: usize) {
        // vt100's scrollback() returns current scroll position
        let current = self.vt.screen().scrollback();

        // Probe for actual scrollback length since it's not exposed
        // This is safe because set_scrollback clamps valid values
        self.vt.screen_mut().set_scrollback(usize::MAX);
        let max_scrollback = self.vt.screen().scrollback();

        // Restore current if we weren't just checking max
        if current < max_scrollback {
            self.vt.screen_mut().set_scrollback(current);
        }

        // Calculate new offset and clamp
        let new_offset = current.saturating_add(lines);
        let clamped_offset = new_offset.min(max_scrollback);

        self.vt.screen_mut().set_scrollback(clamped_offset);
        self.scroll_offset = self.vt.screen().scrollback();
    }

    /// Scroll down (view newer content)
    pub fn scroll_down(&mut self, lines: usize) {
        let current = self.vt.screen().scrollback();
        self.vt
            .screen_mut()
            .set_scrollback(current.saturating_sub(lines));
        self.scroll_offset = self.vt.screen().scrollback();
    }

    /// Scroll to bottom
    pub fn scroll_to_bottom(&mut self) {
        self.vt.screen_mut().set_scrollback(0);
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

    /// Start a new text selection
    pub fn start_selection(&mut self, row: u16, col: u16) {
        self.selection = Some(TextSelection {
            start: (row, col),
            end: (row, col),
        });
        self.is_selecting = true;
    }

    /// Update the selection end point (during drag)
    pub fn update_selection(&mut self, row: u16, col: u16) {
        if let Some(ref mut sel) = self.selection {
            sel.end = (row, col);
        }
    }

    /// Finish selection (mouse released)
    pub fn finish_selection(&mut self) {
        self.is_selecting = false;
    }

    /// Clear the current selection
    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.is_selecting = false;
    }

    /// Check if there is an active selection
    pub fn has_selection(&self) -> bool {
        self.selection.is_some()
    }

    /// Get selected text from the terminal buffer using vt100's optimized method
    pub fn get_selected_text(&self) -> Option<String> {
        let selection = self.selection?;
        let ((start_row, start_col), (end_row, end_col)) = selection.normalized();
        let screen = self.vt.screen();

        // Use vt100's built-in contents_between for much better performance
        let text = screen.contents_between(start_row, start_col, end_row, end_col);

        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    /// Get selection for rendering (returns normalized start/end positions)
    pub fn get_selection_for_render(&self) -> Option<((u16, u16), (u16, u16))> {
        self.selection.map(|s| s.normalized())
    }

    /// Get full scrollback content for search (includes both scrollback and visible screen)
    /// Returns Vec of (scrollback_offset, line_content) where scrollback_offset is how many
    /// lines from the top of scrollback (0 = oldest line in scrollback)
    pub fn get_all_content_for_search(&self) -> Vec<String> {
        let screen = self.vt.screen();
        let (_rows, cols) = screen.size();

        // Build content line by line using the rows() method
        screen.rows(0, cols).collect()
    }

    /// Get maximum scrollback offset (how far we can scroll up)
    pub fn max_scrollback(&self) -> usize {
        // vt100 stores scrollback but doesn't directly expose the length
        // We can probe it by trying to scroll
        // For now, return a large value and let vt100 clamp it
        1000 // This is the value we passed to Parser::new
    }

    /// Scroll to a specific line (for find navigation)
    /// line_offset is relative to top of terminal content
    pub fn scroll_to_line(&mut self, line_offset: usize) {
        // Calculate how much to scroll to show this line
        // The terminal shows `rows` lines, so we want the matched line
        // to be roughly in the middle if possible
        let (rows, _) = self.vt.screen().size();
        let visible_rows = rows as usize;

        // Calculate scrollback position
        // Higher scrollback = older content (scrolled up more)
        if line_offset < visible_rows / 2 {
            self.vt.screen_mut().set_scrollback(line_offset);
        } else {
            self.vt
                .screen_mut()
                .set_scrollback(line_offset.saturating_sub(visible_rows / 2));
        }
        self.scroll_offset = self.vt.screen().scrollback();
    }
    /// Select the word at the given position
    pub fn select_word_at(&mut self, row: u16, col: u16) {
        let screen = self.vt.screen();
        let (_rows, cols) = screen.size();

        if row >= _rows || col >= cols {
            return;
        }

        // Helper to check if a char is a word separator
        let is_separator = |c: char| -> bool {
            c.is_whitespace() || " `~!@#$%^&*()-=+[{]}\\|;:'\",<.>/?".contains(c)
        };

        // Determine if we are starting on a separator or a word character
        let char_under = screen
            .cell(row, col)
            .map(|c| c.contents().chars().next().unwrap_or(' '))
            .unwrap_or(' ');
        let target_is_sep = is_separator(char_under);

        // Find start of the block (word or separator sequence)
        let mut start = col;
        while start > 0 {
            let prev_char = screen
                .cell(row, start - 1)
                .map(|c| c.contents().chars().next().unwrap_or(' '))
                .unwrap_or(' ');

            if is_separator(prev_char) != target_is_sep {
                break;
            }
            start -= 1;
        }

        // Find end of the block
        let mut end = col;
        while end < cols.saturating_sub(1) {
            let next_char = screen
                .cell(row, end + 1)
                .map(|c| c.contents().chars().next().unwrap_or(' '))
                .unwrap_or(' ');

            if is_separator(next_char) != target_is_sep {
                break;
            }
            end += 1;
        }

        self.selection = Some(TextSelection {
            start: (row, start),
            end: (row, end),
        });
        self.is_selecting = false; // Selection is complete (for now)
    }

    /// Select the entire row
    pub fn select_row_at(&mut self, row: u16) {
        let (_rows, cols) = self.vt.screen().size();
        if row >= _rows {
            return;
        }
        self.selection = Some(TextSelection {
            start: (row, 0),
            end: (row, cols.saturating_sub(1)),
        });
        self.is_selecting = false;
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
