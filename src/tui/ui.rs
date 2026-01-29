//! Main UI rendering

use crate::app::{App, RenderState, View};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph};

use super::views;

/// Render the main UI (legacy - kept for compatibility)
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.size();

    // Clear background
    let bg_block = Block::default().style(app.theme.text());
    frame.render_widget(bg_block, area);

    // Layout: Main content + status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    // Render view based on current state
    match app.view {
        View::Connections => views::connections::render(frame, app, chunks[0]),
        View::Session => views::session::render(frame, app, chunks[0]),
        View::Sftp => views::sftp::render(frame, app, chunks[0]),
        View::Tunnels => views::tunnels::render(frame, app, chunks[0]),
        View::Keys => views::keys::render(frame, app, chunks[0]),
        View::Settings => views::settings::render(frame, app, chunks[0]),
        View::Help => views::help::render(frame, app, chunks[0]),
    }

    // Render status bar
    render_status_bar(frame, app, chunks[1]);
}

/// Render the main UI with RenderState (avoids borrow conflicts)
/// Returns the terminal content area if in session view (for mouse coordinate conversion)
pub fn render_with_state(frame: &mut Frame, state: &RenderState) -> Option<Rect> {
    let area = frame.size();

    // Clear background
    let bg_block = Block::default().style(state.theme.text());
    frame.render_widget(bg_block, area);

    // Layout: Main content + optional status bar
    let show_status_bar = state.config.settings.ui.show_status_bar;
    let (main_area, status_area) = if show_status_bar {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),    // Main content
                Constraint::Length(1), // Status bar
            ])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        // Full area for content when status bar is hidden
        (area, None)
    };

    // Track terminal area for session view mouse handling
    let mut terminal_area = None;

    // Render view based on current state
    match state.view {
        View::Connections => views::connections::render_state(frame, state, main_area),
        View::Session => {
            terminal_area = views::session::render_state(frame, state, main_area);
            // Render overlays on top of session view
            if state.session_list_visible {
                views::session_list::render_session_list(frame, state, main_area);
            } else if state.show_connection_overlay {
                views::session_list::render_connection_overlay(frame, state, main_area);
            }
        }
        View::Sftp => views::sftp::render_state(frame, state, main_area),
        View::Tunnels => views::tunnels::render_state(frame, state, main_area),
        View::Keys => views::keys::render_state(frame, state, main_area),
        View::Settings => views::settings::render_state(frame, state, main_area),
        View::Help => views::help::render_state(frame, state, main_area),
    }

    // Render status bar if enabled
    if let Some(status_bar_area) = status_area {
        render_status_bar_state(frame, state, status_bar_area);
    }

    terminal_area
}

/// Render the status bar at the bottom
fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let style = app.theme.status_bar();

    // Left side: View-specific hints
    let hints = match app.view {
        View::Connections => {
            "󰌑 Enter:Connect  e:Edit  n:New  d:Delete  t:Tunnels  f:SFTP  k:Keys  ?:Help"
        }
        View::Session => "󰌑 Shift+Esc:Back  Ctrl+C:Disconnect",
        View::Sftp => "󰌑 Tab:Switch  Enter:Open  c:Copy  m:Move  d:Delete  Esc:Back",
        View::Tunnels => "󰌑 Enter:Toggle  n:New  d:Delete  Esc:Back",
        View::Keys => "󰌑 Enter:View  n:Generate  i:Import  d:Delete  Esc:Back",
        View::Settings => "󰌑 Enter:Edit  Esc:Back",
        View::Help => "󰌑 Esc/q/?:Close",
    };

    // Right side: Session count and status
    let session_count = app.sessions.list().len();
    let status_right = format!("Sessions: {} │ Ctrl+Q:Quit  ", session_count);

    // Calculate spacing
    let hints_len = hints.len() + 1;
    let right_len = status_right.len();
    let spacing = area
        .width
        .saturating_sub(hints_len as u16 + right_len as u16);

    // Navigation arrows
    let back_style = if !app.view_back_history.is_empty() {
        style
    } else {
        style.fg(Color::DarkGray)
    };
    let fwd_style = if !app.view_forward_history.is_empty() {
        style
    } else {
        style.fg(Color::DarkGray)
    };

    let spans = vec![
        Span::styled(" ", style),
        Span::styled("<", back_style),
        Span::styled(" ", style),
        Span::styled(">", fwd_style),
        Span::styled("  ", style),
        Span::styled(hints, style),
        Span::styled(" ".repeat(spacing.saturating_sub(6) as usize), style), // Adjust spacing for arrows (approx 6 chars)
        Span::styled(&status_right, style),
    ];

    let paragraph = Paragraph::new(Line::from(spans)).style(style);

    frame.render_widget(paragraph, area);

    // Show status message if present
    if let Some(msg) = &app.status_message {
        let msg_area = Rect::new(
            area.x + area.width.saturating_sub(msg.len() as u16 + 2),
            area.y,
            msg.len() as u16 + 2,
            1,
        );
        let msg_widget = Paragraph::new(format!(" {} ", msg)).style(app.theme.warning());
        frame.render_widget(msg_widget, msg_area);
    }
}

/// Render the status bar with RenderState
fn render_status_bar_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let style = state.theme.status_bar();
    let kb = state.icons.keyboard;

    // Left side: View-specific hints (using dynamic keyboard icon)
    let hints = match state.view {
        View::Connections => format!(
            "{}Enter:Connect  e:Edit  n:New  d:Delete  t:Tunnels  f:SFTP  K:Keys  ?:Help",
            kb
        ),
        View::Session => {
            if state.escape_prefix_active {
                format!("{}Ctrl+B: n:Next p:Prev l:List c:Connect w:Close", kb)
            } else {
                format!("{}Ctrl+B:Prefix  Alt+Left:Back  Shift+F:SFTP", kb)
            }
        }
        View::Sftp => format!(
            "{}Tab:Switch  Enter:Open  c:Copy  m:Move  d:Delete  Esc:Back",
            kb
        ),
        View::Tunnels => format!("{}Enter:Toggle  n:New  d:Delete  Esc:Back", kb),
        View::Keys => format!("{}Enter:View  n:Generate  i:Import  d:Delete  Esc:Back", kb),
        View::Settings => format!("{}Enter:Edit  Esc:Back", kb),
        View::Help => format!("{}Esc/q/?:Close", kb),
    };

    // Right side: Session count and status
    let session_count = state.sessions.len();
    let status_right = format!("Sessions: {} │ Ctrl+Q:Quit  ", session_count);

    // Calculate spacing
    let hints_len = hints.len() + 1;
    let right_len = status_right.len();
    let spacing = area
        .width
        .saturating_sub(hints_len as u16 + right_len as u16);

    // Navigation arrows
    let back_style = if state.can_go_back {
        style
    } else {
        style.fg(Color::DarkGray)
    };
    let fwd_style = if state.can_go_forward {
        style
    } else {
        style.fg(Color::DarkGray)
    };

    let spans = vec![
        Span::styled(" ", style),
        Span::styled("<", back_style),
        Span::styled(" ", style),
        Span::styled(">", fwd_style),
        Span::styled("  ", style),
        Span::styled(hints, style),
        Span::styled(" ".repeat(spacing.saturating_sub(6) as usize), style), // Adjust spacing
        Span::styled(&status_right, style),
    ];

    let paragraph = Paragraph::new(Line::from(spans)).style(style);

    frame.render_widget(paragraph, area);

    // Show status message if present
    if let Some(msg) = &state.status_message {
        let msg_area = Rect::new(
            area.x + area.width.saturating_sub(msg.len() as u16 + 2),
            area.y,
            msg.len() as u16 + 2,
            1,
        );
        let msg_widget = Paragraph::new(format!(" {} ", msg)).style(state.theme.warning());
        frame.render_widget(msg_widget, msg_area);
    }
}
