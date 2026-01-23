//! Session list overlay for multi-session switching

use crate::app::RenderState;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Padding};

/// Width of the session list panel (percentage of screen width)
const PANEL_WIDTH_PERCENT: u16 = 30;
/// Minimum panel width in characters
const MIN_PANEL_WIDTH: u16 = 30;

/// Render the session list overlay
pub fn render_session_list(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;
    
    // Calculate panel width
    let panel_width = (area.width * PANEL_WIDTH_PERCENT / 100).max(MIN_PANEL_WIDTH).min(area.width);
    
    // Panel positioned on the left side
    let panel_area = Rect {
        x: area.x,
        y: area.y,
        width: panel_width,
        height: area.height,
    };
    
    // Clear the area first (for overlay effect)
    frame.render_widget(Clear, panel_area);
    
    // Create the panel
    let block = Block::default()
        .title(" 󰆍 Sessions ")
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .style(Style::default().bg(theme.bg_panel()))
        .padding(Padding::horizontal(1));
    
    let inner = block.inner(panel_area);
    frame.render_widget(block, panel_area);
    
    // Build session list items
    let items: Vec<ListItem> = state.session_order
        .iter()
        .enumerate()
        .map(|(idx, &session_id)| {
            let session = state.sessions.iter().find(|s| s.id == session_id);
            let name = session.map(|s| s.name.as_str()).unwrap_or("Unknown");
            let is_selected = idx == state.session_list_selected;
            let is_active = Some(session_id) == state.active_session;
            
            // Format: [1] session-name (● if active)
            let mut spans = vec![
                Span::styled(
                    format!("[{}] ", idx + 1),
                    if is_selected { theme.key_hint() } else { theme.text_dim() }
                ),
            ];
            
            if is_active {
                spans.push(Span::styled("● ", Style::default().fg(Color::Green)));
            }
            
            spans.push(Span::styled(
                name.to_string(),
                if is_selected { theme.selected() } else { theme.text() }
            ));
            
            ListItem::new(Line::from(spans))
        })
        .collect();
    
    if items.is_empty() {
        let empty_text = Line::from(vec![
            Span::styled("No active sessions", theme.text_dim())
        ]);
        frame.render_widget(
            ratatui::widgets::Paragraph::new(empty_text),
            inner
        );
    } else {
        let list = List::new(items)
            .highlight_style(theme.selected())
            .highlight_symbol("> ");
        
        frame.render_widget(list, inner);
    }
    
    // Help line at bottom
    let help_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(2),
        width: inner.width,
        height: 2,
    };
    
    let help = ratatui::widgets::Paragraph::new(vec![
        Line::from(vec![
            Span::styled("↑/↓", theme.key_hint()),
            Span::styled(" select  ", theme.text_dim()),
            Span::styled("Enter", theme.key_hint()),
            Span::styled(" switch", theme.text_dim()),
        ]),
        Line::from(vec![
            Span::styled("Esc", theme.key_hint()),
            Span::styled(" close   ", theme.text_dim()),
            Span::styled("1-9", theme.key_hint()),
            Span::styled(" jump", theme.text_dim()),
        ]),
    ]);
    
    frame.render_widget(help, help_area);
}

/// Render the connection overlay (host list for new connection)
pub fn render_connection_overlay(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;
    
    // Calculate panel width
    let panel_width = (area.width * PANEL_WIDTH_PERCENT / 100).max(MIN_PANEL_WIDTH).min(area.width);
    
    // Panel positioned on the left side
    let panel_area = Rect {
        x: area.x,
        y: area.y,
        width: panel_width,
        height: area.height,
    };
    
    // Clear the area first
    frame.render_widget(Clear, panel_area);
    
    // Create the panel
    let block = Block::default()
        .title("  New Connection ")
        .title_style(theme.title())
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .style(Style::default().bg(theme.bg_panel()))
        .padding(Padding::horizontal(1));
    
    let inner = block.inner(panel_area);
    frame.render_widget(block, panel_area);
    
    // Build host list items
    let all_hosts: Vec<_> = state.config.hosts.iter().collect();
    let items: Vec<ListItem> = all_hosts
        .iter()
        .enumerate()
        .map(|(idx, host)| {
            let is_selected = idx == state.selected_host_index;
            
            let style = if is_selected {
                theme.selected()
            } else {
                theme.text()
            };
            
            let line = Line::from(vec![
                Span::styled(format!(" {} ", host.name), style),
            ]);
            
            ListItem::new(line)
        })
        .collect();
    
    if items.is_empty() {
        let empty_text = Line::from(vec![
            Span::styled("No hosts configured", theme.text_dim())
        ]);
        frame.render_widget(
            ratatui::widgets::Paragraph::new(empty_text),
            inner
        );
    } else {
        let list = List::new(items)
            .highlight_style(theme.selected())
            .highlight_symbol("> ");
        
        frame.render_widget(list, inner);
    }
    
    // Help line at bottom
    let help_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1),
        width: inner.width,
        height: 1,
    };
    
    let help = ratatui::widgets::Paragraph::new(Line::from(vec![
        Span::styled("↑/↓", theme.key_hint()),
        Span::styled(" select  ", theme.text_dim()),
        Span::styled("Enter", theme.key_hint()),
        Span::styled(" connect  ", theme.text_dim()),
        Span::styled("Esc", theme.key_hint()),
        Span::styled(" cancel", theme.text_dim()),
    ]));
    
    frame.render_widget(help, help_area);
}
