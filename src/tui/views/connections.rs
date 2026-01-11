//! Connections view - host list and management

use crate::app::{App, RenderState};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Padding};

/// Render the connections view with RenderState
pub fn render_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;
    
    // Main layout with side panel
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70),  // Host list
            Constraint::Percentage(30),  // Details panel
        ])
        .split(area);
    
    // Host list panel
    render_host_list_state(frame, state, chunks[0]);
    
    // Details panel
    render_details_panel_state(frame, state, chunks[1]);
}

fn render_host_list_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;
    let icons = &state.icons;
    
    let title = Line::from(vec![
        Span::styled(format!(" {} ", icons.connections), theme.title()),
        Span::styled("Connections", theme.title()),
        Span::styled(" ", theme.title()),
    ]);
    
    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(theme.bg_panel()));
    
    let inner = block.inner(area);
    frame.render_widget(block, area);
    
    // Build list items from config - track host index
    let mut items: Vec<ListItem> = Vec::new();
    let mut host_idx: usize = 0;
    
    for group in &state.config.groups {
        if group.expanded {
            for host in &group.hosts {
                let is_selected = host_idx == state.selected_host_index;
                let line = format_host_line_with_selection(host, theme, icons, is_selected);
                items.push(ListItem::new(line));
                host_idx += 1;
            }
        }
    }
    
    for host in &state.config.hosts {
        let is_selected = host_idx == state.selected_host_index;
        let line = format_host_line_with_selection(host, theme, icons, is_selected);
        items.push(ListItem::new(line));
        host_idx += 1;
    }
    
    if items.is_empty() {
        let empty_text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  No connections configured", theme.text_dim()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Add hosts to ", theme.text_dim()),
                Span::styled("~/.config/rustyssh/config.yaml", theme.key_hint()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Example:", theme.text_dim()),
            ]),
            Line::from(vec![
                Span::styled("  hosts:", theme.text_dim()),
            ]),
            Line::from(vec![
                Span::styled("    - name: myserver", theme.text_dim()),
            ]),
            Line::from(vec![
                Span::styled("      hostname: 192.168.1.100", theme.text_dim()),
            ]),
            Line::from(vec![
                Span::styled("      username: user", theme.text_dim()),
            ]),
        ];
        let empty = Paragraph::new(empty_text);
        frame.render_widget(empty, inner);
    } else {
        let list = List::new(items);
        frame.render_widget(list, inner);
    }
}

/// Format a single host line with selection state
fn format_host_line_with_selection(
    host: &crate::config::HostConfig, 
    theme: &crate::tui::Theme, 
    icons: &crate::tui::Icons,
    is_selected: bool
) -> Line<'static> {
    // Status indicator
    let status_icon = icons.disconnected;
    
    // Auth method icon
    let auth_icon = match &host.auth {
        crate::config::AuthMethod::Password => icons.password,
        crate::config::AuthMethod::KeyFile { .. } => icons.key_file,
        crate::config::AuthMethod::Agent => icons.agent,
        crate::config::AuthMethod::Certificate { .. } => icons.certificate,
    };
    
    let (prefix, name_style, conn_style) = if is_selected {
        ("▶ ", theme.selected(), theme.selected())
    } else {
        ("  ", theme.text_bright(), theme.text_dim())
    };
    
    Line::from(vec![
        Span::styled(prefix.to_string(), if is_selected { theme.selected() } else { theme.text() }),
        Span::styled(status_icon.to_string(), theme.text_dim()),
        Span::styled(auth_icon.to_string(), Style::default().fg(theme.accent_info())),
        Span::styled(host.name.clone(), name_style),
        Span::styled(format!("  {}", host.connection_string()), conn_style),
    ])
}

fn render_details_panel_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;
    let icons = &state.icons;
    
    let title = Line::from(vec![
        Span::styled(format!(" {} ", icons.info), theme.title()),
        Span::styled("Details", theme.title()),
        Span::styled(" ", theme.title()),
    ]);
    
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_normal())
        .padding(Padding::uniform(1))
        .style(Style::default().bg(theme.bg_panel()));
    
    let inner = block.inner(area);
    frame.render_widget(block, area);
    
    let content = vec![
        Line::from(vec![
            Span::styled("Quick Start", Style::default().add_modifier(Modifier::BOLD).fg(theme.fg_bright())),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("󰌑 ", Style::default().fg(theme.accent_primary())),
            Span::styled("n", theme.key_hint()),
            Span::styled(" - New connection", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("󰌑 ", Style::default().fg(theme.accent_primary())),
            Span::styled("Enter", theme.key_hint()),
            Span::styled(" - Connect", theme.text()),
        ]),
    ];
    
    let paragraph = Paragraph::new(content);
    frame.render_widget(paragraph, inner);
}

/// Render the connections view
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    
    // Main layout with side panel
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70),  // Host list
            Constraint::Percentage(30),  // Details panel
        ])
        .split(area);
    
    // Host list panel
    render_host_list(frame, app, chunks[0]);
    
    // Details panel
    render_details_panel(frame, app, chunks[1]);
}

/// Render the host list
fn render_host_list(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    
    // Title with icon
    let title = Line::from(vec![
        Span::styled(" 󰢹 ", theme.title()),
        Span::styled("Connections", theme.title()),
        Span::styled(" ", theme.title()),
    ]);
    
    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(theme.bg_panel()));
    
    let inner = block.inner(area);
    frame.render_widget(block, area);
    
    // Build list items from config
    let mut items: Vec<ListItem> = Vec::new();
    
    // Groups
    for group in &app.config.groups {
        // Group header
        let icon = if group.expanded { "󰅀 " } else { "󰅂 " };
        let group_line = Line::from(vec![
            Span::styled(icon, theme.accent_secondary()),
            Span::styled(&group.name, Style::default().add_modifier(Modifier::BOLD).fg(theme.fg_bright())),
        ]);
        items.push(ListItem::new(group_line));
        
        // Group hosts (if expanded)
        if group.expanded {
            for host in &group.hosts {
                let line = format_host_line(host, theme);
                items.push(ListItem::new(line));
            }
        }
    }
    
    // Ungrouped hosts
    if !app.config.hosts.is_empty() {
        let ungrouped_line = Line::from(vec![
            Span::styled("󰅀 ", theme.accent_secondary()),
            Span::styled("Ungrouped", Style::default().add_modifier(Modifier::BOLD).fg(theme.fg_bright())),
        ]);
        items.push(ListItem::new(ungrouped_line));
        
        for host in &app.config.hosts {
            let line = format_host_line(host, theme);
            items.push(ListItem::new(line));
        }
    }
    
    // Empty state
    if items.is_empty() {
        let empty_text = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  No connections configured", theme.text_dim()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Press ", theme.text_dim()),
                Span::styled("n", theme.key_hint()),
                Span::styled(" to add a new host", theme.text_dim()),
            ]),
        ];
        let empty = Paragraph::new(empty_text);
        frame.render_widget(empty, inner);
    } else {
        let list = List::new(items)
            .highlight_style(theme.selected())
            .highlight_symbol("▶ ");
        
        frame.render_widget(list, inner);
    }
}

/// Format a single host line
fn format_host_line(host: &crate::config::HostConfig, theme: &crate::tui::Theme) -> Line<'static> {
    // Status indicator
    let status_icon = "○ "; // ● for connected, ○ for disconnected
    
    // Auth method icon
    let auth_icon = match &host.auth {
        crate::config::AuthMethod::Password => "󰌆 ",
        crate::config::AuthMethod::KeyFile { .. } => "󰌋 ",
        crate::config::AuthMethod::Agent => "󰌉 ",
        crate::config::AuthMethod::Certificate { .. } => "󰄤 ",
    };
    
    Line::from(vec![
        Span::raw("    "),
        Span::styled(status_icon, theme.text_dim()),
        Span::styled(auth_icon, theme.accent_info()),
        Span::styled(host.name.clone(), theme.text_bright()),
        Span::styled(format!("  {}", host.connection_string()), theme.text_dim()),
    ])
}

/// Render the details panel
fn render_details_panel(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    
    let title = Line::from(vec![
        Span::styled(" 󰋼 ", theme.title()),
        Span::styled("Details", theme.title()),
        Span::styled(" ", theme.title()),
    ]);
    
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_normal())
        .padding(Padding::uniform(1))
        .style(Style::default().bg(theme.bg_panel()));
    
    let inner = block.inner(area);
    frame.render_widget(block, area);
    
    // Show quickstart guide if no hosts
    let content = vec![
        Line::from(vec![
            Span::styled("Quick Start", Style::default().add_modifier(Modifier::BOLD).fg(theme.fg_bright())),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("󰌑 ", theme.accent_primary()),
            Span::styled("n", theme.key_hint()),
            Span::styled(" - New connection", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("󰌑 ", theme.accent_primary()),
            Span::styled("e", theme.key_hint()),
            Span::styled(" - Edit selected", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("󰌑 ", theme.accent_primary()),
            Span::styled("Enter", theme.key_hint()),
            Span::styled(" - Connect", theme.text()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Views", Style::default().add_modifier(Modifier::BOLD).fg(theme.fg_bright())),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("󰌑 ", theme.accent_primary()),
            Span::styled("t", theme.key_hint()),
            Span::styled(" - Tunnels", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("󰌑 ", theme.accent_primary()),
            Span::styled("f", theme.key_hint()),
            Span::styled(" - SFTP Browser", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("󰌑 ", theme.accent_primary()),
            Span::styled("k", theme.key_hint()),
            Span::styled(" - Key Manager", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("󰌑 ", theme.accent_primary()),
            Span::styled("s", theme.key_hint()),
            Span::styled(" - Settings", theme.text()),
        ]),
    ];
    
    let paragraph = Paragraph::new(content);
    frame.render_widget(paragraph, inner);
}
