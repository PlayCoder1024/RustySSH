//! SFTP file browser view

use crate::app::{App, RenderState};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, Cell, Padding};

/// Render the SFTP view with RenderState
pub fn render_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(area);
    
    let pane_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(chunks[0]);
    
    // Left pane
    render_file_pane_state(frame, state, pane_chunks[0], true, true);
    render_file_pane_state(frame, state, pane_chunks[1], false, false);
    
    // Transfer queue
    let title = Line::from(vec![
        Span::styled(" 󰇚 ", theme.title()),
        Span::styled("Transfers", theme.title()),
    ]);
    
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_normal())
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(theme.bg_panel()));
    
    let inner = block.inner(chunks[1]);
    frame.render_widget(block, chunks[1]);
    
    let content = Line::from(vec![
        Span::styled("No active transfers", theme.text_dim()),
    ]);
    frame.render_widget(Paragraph::new(content), inner);
}

fn render_file_pane_state(frame: &mut Frame, state: &RenderState, area: Rect, is_left: bool, is_active: bool) {
    let theme = &state.theme;
    let label = if is_left { "Local" } else { "Remote" };
    
    let title = Line::from(vec![
        Span::styled(" 󰉋 ", theme.title()),
        Span::styled(label, theme.title()),
    ]);
    
    let border_style = if is_active { theme.border_focus() } else { theme.border_normal() };
    
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(theme.bg_panel()));
    
    let inner = block.inner(area);
    frame.render_widget(block, area);
    
    let rows = vec![
        Row::new(vec![
            Cell::from("📁 ..").style(theme.text()),
            Cell::from("<DIR>").style(theme.text_dim()),
        ]),
        Row::new(vec![
            Cell::from("📁 Documents").style(theme.text()),
            Cell::from("<DIR>").style(theme.text_dim()),
        ]),
    ];
    
    let widths = [Constraint::Min(20), Constraint::Length(8)];
    let table = Table::new(rows, widths).highlight_style(theme.selected());
    frame.render_widget(table, inner);
}

/// Render the SFTP view
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    
    // Dual pane layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // File panes
            Constraint::Length(3), // Transfer queue
        ])
        .split(area);
    
    let pane_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(chunks[0]);
    
    // Left pane (local)
    render_file_pane(frame, app, pane_chunks[0], true, true);
    
    // Right pane (remote)
    render_file_pane(frame, app, pane_chunks[1], false, false);
    
    // Transfer queue
    render_transfer_queue(frame, app, chunks[1]);
}

/// Render a file pane
fn render_file_pane(frame: &mut Frame, app: &App, area: Rect, is_left: bool, is_active: bool) {
    let theme = &app.theme;
    
    let label = if is_left { "Local" } else { "Remote" };
    let path = if is_left { "/home/user" } else { "/" }; // Placeholder
    
    let title = Line::from(vec![
        Span::styled(" 󰉋 ", theme.title()),
        Span::styled(format!("{} ({})", label, path), theme.title()),
        Span::styled(" ", theme.title()),
    ]);
    
    let border_style = if is_active {
        theme.border_focus()
    } else {
        theme.border_normal()
    };
    
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(theme.bg_panel()));
    
    let inner = block.inner(area);
    frame.render_widget(block, area);
    
    // Sample file listing
    let rows = vec![
        create_file_row("..", "<DIR>", "", theme, false),
        create_file_row("Documents", "<DIR>", "2024-01-10", theme, false),
        create_file_row("Downloads", "<DIR>", "2024-01-09", theme, false),
        create_file_row("config.yaml", "2.1K", "2024-01-08", theme, true),
        create_file_row("notes.md", "512B", "2024-01-07", theme, false),
    ];
    
    let widths = [
        Constraint::Min(20),
        Constraint::Length(8),
        Constraint::Length(12),
    ];
    
    let table = Table::new(rows, widths)
        .header(Row::new(vec![
            Cell::from("Name").style(theme.text_dim()),
            Cell::from("Size").style(theme.text_dim()),
            Cell::from("Modified").style(theme.text_dim()),
        ]))
        .highlight_style(theme.selected());
    
    frame.render_widget(table, inner);
}

/// Create a file row
fn create_file_row<'a>(name: &'a str, size: &'a str, date: &'a str, theme: &crate::tui::Theme, selected: bool) -> Row<'a> {
    let icon = if size == "<DIR>" { "📁 " } else { "📄 " };
    
    let style = if selected {
        theme.selected()
    } else {
        theme.text()
    };
    
    Row::new(vec![
        Cell::from(format!("{}{}", icon, name)).style(style),
        Cell::from(size).style(theme.text_dim()),
        Cell::from(date).style(theme.text_dim()),
    ])
}

/// Render the transfer queue
fn render_transfer_queue(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    
    let title = Line::from(vec![
        Span::styled(" 󰇚 ", theme.title()),
        Span::styled("Transfers", theme.title()),
        Span::styled(" ", theme.title()),
    ]);
    
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_normal())
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(theme.bg_panel()));
    
    let inner = block.inner(area);
    frame.render_widget(block, area);
    
    // Sample transfer status
    let content = Line::from(vec![
        Span::styled("No active transfers", theme.text_dim()),
        Span::raw("  │  "),
        Span::styled("Press ", theme.text_dim()),
        Span::styled("c", theme.key_hint()),
        Span::styled(" to copy,  ", theme.text_dim()),
        Span::styled("m", theme.key_hint()),
        Span::styled(" to move", theme.text_dim()),
    ]);
    
    let paragraph = Paragraph::new(content);
    frame.render_widget(paragraph, inner);
}
