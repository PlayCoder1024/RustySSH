//! Terminal UI module

mod theme;
pub mod ui;
pub mod views;
pub mod widgets;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io::{self, stdout, Stdout};

pub use theme::Theme;
pub use ui::{render, render_with_state};

/// Terminal wrapper for ratatui
pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Tui {
    /// Create a new terminal UI
    pub fn new() -> Result<Self> {
        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;
        
        Ok(Self { terminal })
    }

    /// Enter TUI mode
    pub fn enter(&mut self) -> Result<()> {
        enable_raw_mode()?;
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableMouseCapture
        )?;
        
        // Clear and hide cursor
        self.terminal.clear()?;
        
        Ok(())
    }

    /// Exit TUI mode
    pub fn exit(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        
        self.terminal.show_cursor()?;
        
        Ok(())
    }

    /// Draw the terminal
    pub fn draw<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Frame),
    {
        self.terminal.draw(f)?;
        Ok(())
    }

    /// Get terminal size
    pub fn size(&self) -> Result<Rect> {
        Ok(self.terminal.size()?)
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        // Best effort cleanup
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
    }
}
