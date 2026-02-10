//! SSH session view with terminal

use crate::app::{App, RenderState};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};

/// Render the session view with RenderState
/// Returns the terminal content area for mouse coordinate conversion
pub fn render_state(frame: &mut Frame, state: &RenderState, area: Rect) -> Option<Rect> {
    let theme = &state.theme;

    if state.sessions.is_empty() {
        let block = Block::default()
            .title(" 󰆍 Terminal ")
            .borders(Borders::ALL)
            .border_style(theme.border_focus())
            .style(Style::default().bg(theme.bg_panel()));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let text = vec![
            Line::from(""),
            Line::from(vec![Span::styled("  No active sessions", theme.text_dim())]),
        ];
        let paragraph = Paragraph::new(text);
        frame.render_widget(paragraph, inner);
        return None;
    }

    // Layout: tabs + terminal
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    // Render tabs
    let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let titles: Vec<Line> = state
        .sessions
        .iter()
        .map(|session| {
            let is_active = state.active_session == Some(session.id);

            // Base style
            let style = if is_active {
                theme.selected().add_modifier(Modifier::BOLD)
            } else {
                theme.text_dim()
            };

            let mut spans = Vec::new();

            // Status indicator / Icon
            if session.status == crate::ssh::SessionStatus::Connecting {
                let frame = spinner_frames[state.frame_count % spinner_frames.len()];
                spans.push(Span::styled(format!(" {} ", frame), theme.accent_warning()));
            } else if let Some(progress) = session.progress {
                if progress < 0.0 {
                    // Indeterminate progress — show spinner
                    let frame = spinner_frames[state.frame_count % spinner_frames.len()];
                    spans.push(Span::styled(format!(" {} ", frame), theme.accent_info()));
                } else {
                    // Progress pie chart (Nerd Font circle slices to match icon size)
                    let pie_frames = ["󰝦", "󰪞", "󰪠", "󰪣", "󰪥", "󰝥"];
                    let idx =
                        (progress.clamp(0.0, 1.0) * (pie_frames.len() - 1) as f32).round() as usize;
                    spans.push(Span::styled(
                        format!(" {} ", pie_frames[idx]),
                        theme.accent_info(),
                    ));
                }
            } else {
                // Static icon
                let icon = if is_active { " " } else { " " };
                spans.push(Span::styled(
                    format!(" {} ", icon),
                    if is_active {
                        Style::default().fg(theme.accent_primary())
                    } else {
                        theme.text_dim()
                    },
                ));
            }

            // Name
            spans.push(Span::styled(format!("{} ", session.name), style));

            Line::from(spans)
        })
        .collect();

    let selected = state
        .sessions
        .iter()
        .position(|s| state.active_session == Some(s.id))
        .unwrap_or(0);

    let tabs = Tabs::new(titles)
        .select(selected)
        .style(theme.text())
        .highlight_style(theme.selected());

    frame.render_widget(tabs, chunks[0]);

    // Render terminal
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .style(Style::default().bg(theme.bg_main()));

    let inner = block.inner(chunks[1]);
    frame.render_widget(block, chunks[1]);

    // Get active session content with native terminal colors
    if let Some(session_id) = state.active_session {
        if let Some(session) = state.sessions.iter().find(|s| s.id == session_id) {
            // Use pre-rendered styled lines with full ANSI color support
            let lines: Vec<Line> = session
                .styled_lines
                .iter()
                .take(inner.height as usize)
                .cloned()
                .collect();

            let paragraph = Paragraph::new(lines);
            frame.render_widget(paragraph, inner);

            if session.cursor_visible && !state.find_overlay_visible {
                let (cursor_row, cursor_col) = session.cursor_position;
                let cursor_x = inner.x + cursor_col;
                let cursor_y = inner.y + cursor_row;

                if cursor_x < inner.x + inner.width && cursor_y < inner.y + inner.height {
                    frame.set_cursor(cursor_x, cursor_y);
                }
            }
        }
    }

    // Render find overlay if visible
    if state.find_overlay_visible {
        use crate::tui::widgets::render_find_overlay;
        render_find_overlay(
            frame,
            area,
            &state.find_query,
            state.find_match_index,
            state.find_match_count,
            theme,
        );
    }

    Some(inner)
}

/// Render the session view
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let _theme = &app.theme;

    // Get active sessions
    let sessions = app.sessions.list();

    if sessions.is_empty() {
        // No active sessions
        render_no_sessions(frame, app, area);
        return;
    }

    // Layout: tabs + terminal
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Tab bar
            Constraint::Min(1),    // Terminal content
        ])
        .split(area);

    // Render tab bar
    render_tabs(frame, app, chunks[0]);

    // Render terminal content
    render_terminal(frame, app, chunks[1]);
}

/// Render when no sessions are active
fn render_no_sessions(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let block = Block::default()
        .title(" 󰆍 Terminal ")
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = vec![
        Line::from(""),
        Line::from(vec![Span::styled("  No active sessions", theme.text_dim())]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Press ", theme.text_dim()),
            Span::styled("Esc", theme.key_hint()),
            Span::styled(" to return to connections", theme.text_dim()),
        ]),
    ];

    let paragraph = Paragraph::new(text);
    frame.render_widget(paragraph, inner);
}

/// Render session tabs
fn render_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let sessions = app.sessions.list();

    let titles: Vec<Line> = sessions
        .iter()
        .enumerate()
        .map(|(_i, session)| {
            let is_active = app.active_session == Some(session.id);
            let style = if is_active {
                theme.selected()
            } else {
                theme.text()
            };

            Line::from(vec![Span::styled(format!(" {} ", session.name), style)])
        })
        .collect();

    // Find selected index
    let selected = sessions
        .iter()
        .position(|s| app.active_session == Some(s.id))
        .unwrap_or(0);

    let tabs = Tabs::new(titles)
        .select(selected)
        .style(theme.text())
        .highlight_style(theme.selected())
        .divider(Span::styled("│", theme.text_dim()));

    frame.render_widget(tabs, area);
}

/// Render terminal content
fn render_terminal(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .style(Style::default().bg(theme.bg_main()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Get active session content with native ANSI colors
    if let Some(session_id) = app.active_session {
        if let Some(session) = app.sessions.get(session_id) {
            use crate::tui::terminal_render::render_screen_to_lines;

            // Render VT100 screen with full color support
            let screen = session.screen();
            let styled_lines = render_screen_to_lines(screen);

            // Apply keyword highlighting on top of VT100 colors
            let lines: Vec<Line> = styled_lines
                .into_iter()
                .take(inner.height as usize)
                .map(|line| app.highlighter.highlight_styled_line(line))
                .collect();

            let paragraph = Paragraph::new(lines);

            frame.render_widget(paragraph, inner);

            // Show cursor if visible
            if session.cursor_visible() {
                let (cursor_row, cursor_col) = session.cursor_position();
                let cursor_x = inner.x + cursor_col;
                let cursor_y = inner.y + cursor_row;

                if cursor_x < inner.x + inner.width && cursor_y < inner.y + inner.height {
                    frame.set_cursor(cursor_x, cursor_y);
                }
            }
        }
    }
}
