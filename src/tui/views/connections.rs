//! Connections view - host list and management

use crate::app::{App, RenderState};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Padding, Paragraph};

/// Render the connections view with RenderState
pub fn render_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;

    // Vertical layout: Banner at top, then main content
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Banner
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Status bar
        ])
        .split(area);

    // Render ASCII banner
    render_banner(frame, theme, main_chunks[0]);

    // Horizontal layout for host list and details
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55), // Host list
            Constraint::Percentage(45), // Details panel
        ])
        .split(main_chunks[1]);

    // Host list panel
    render_host_list_state(frame, state, content_chunks[0]);

    // Details panel
    render_details_panel_state(frame, state, content_chunks[1]);

    // Status bar at bottom
    render_status_bar(frame, state, main_chunks[2]);

    // Render connecting overlay if connecting
    if let Some(host_name) = &state.connecting_to_host {
        render_connecting_overlay(frame, theme, area, host_name, state.connection_start_time);
    }

    // Render host search overlay if searching
    if state.host_search_visible {
        render_host_search_overlay(frame, state, area);
    }
}

/// Render the ASCII art banner
fn render_banner(frame: &mut Frame, theme: &crate::tui::Theme, area: Rect) {
    // Banner with precise alignment
    // Note: ⚡ and 🦀 emojis are 2 cells wide each in terminals
    // Total line width: 71 display cells
    // Border line: "  ╭" + 65×"─" + "╮" = 71 cells
    // Content: "  │" + 65 cells of content + "│" = 71 cells

    let banner_text = vec![
        // Line 1: Top border (71 cells: 2 spaces + ╭ + 65 dashes + ╮)
        Line::from(vec![Span::styled(
            "  ╭─────────────────────────────────────────────────────────────────╮",
            Style::default().fg(theme.accent_primary()),
        )]),
        // Line 2: RUSTY + SSH + tagline (content = 65 cells)
        // "  │" (3) + content (65) + "│" (1) = 69 + 2 leading spaces = 71
        Line::from(vec![
            Span::styled("  │ ", Style::default().fg(theme.accent_primary())),
            Span::styled(
                "█▀█ █ █ █▀ ▀█▀ █▄█",
                Style::default()
                    .fg(theme.accent_info())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ╱╱ ", Style::default().fg(theme.accent_primary())),
            Span::styled(
                "█▀ █▀ █ █",
                Style::default()
                    .fg(theme.accent_secondary())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("    ", Style::default().fg(theme.accent_primary())),
            Span::styled("⚡", Style::default().fg(theme.accent_warning())),
            Span::styled(
                " SECURE SHELL ",
                Style::default().fg(theme.accent_warning()),
            ),
            Span::styled("⚡", Style::default().fg(theme.accent_warning())),
            Span::styled("           │", Style::default().fg(theme.accent_primary())),
        ]),
        // Line 3: Second row of ASCII art + version (content = 65 cells)
        Line::from(vec![
            Span::styled("  │ ", Style::default().fg(theme.accent_primary())),
            Span::styled(
                "█▀▄ █▄█ ▄█  █  ░█░",
                Style::default()
                    .fg(theme.accent_info())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ╱╱ ", Style::default().fg(theme.accent_primary())),
            Span::styled(
                "▄█ ▄█ █▀█",
                Style::default()
                    .fg(theme.accent_secondary())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "        v0.2.0 ",
                Style::default().fg(theme.accent_primary()),
            ),
            Span::styled("🦀", Style::default().fg(theme.accent_warning())),
            Span::styled(
                "                │",
                Style::default().fg(theme.accent_primary()),
            ),
        ]),
        // Line 4: Bottom border (71 cells)
        Line::from(vec![Span::styled(
            "  ╰─────────────────────────────────────────────────────────────────╯",
            Style::default().fg(theme.accent_primary()),
        )]),
    ];

    let paragraph = Paragraph::new(banner_text).style(Style::default().bg(theme.bg_panel()));

    frame.render_widget(paragraph, area);
}

/// Render the status bar at the bottom
fn render_status_bar(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;

    // Count hosts and sessions
    let total_hosts = state.config.hosts.len()
        + state
            .config
            .groups
            .iter()
            .map(|g| g.hosts.len())
            .sum::<usize>();
    let active_sessions = state.sessions.len();

    // Build status info
    let active_color = if active_sessions > 0 {
        theme.accent_success()
    } else {
        theme.fg_dim()
    };
    let status_left = vec![
        Span::styled(" 󰢹 ", Style::default().fg(theme.accent_primary())),
        Span::styled(format!("{} hosts", total_hosts), theme.text()),
        Span::styled(" │ ", theme.text_dim()),
        Span::styled("󱘖 ", Style::default().fg(active_color)),
        Span::styled(
            format!("{} active", active_sessions),
            Style::default().fg(active_color),
        ),
    ];

    // Matrix-style decorations
    let matrix_chars = "░▒▓█▓▒░";
    let status_right = vec![
        Span::styled(
            matrix_chars,
            Style::default()
                .fg(theme.accent_primary())
                .add_modifier(Modifier::DIM),
        ),
        Span::styled(" ", theme.text()),
        Span::styled("v0.2.0", Style::default().fg(theme.accent_info())),
        Span::styled(" ", theme.text()),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(theme.accent_primary())
                .add_modifier(Modifier::DIM),
        )
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner area for left and right alignment
    let inner_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let left_para = Paragraph::new(Line::from(status_left));
    let right_para = Paragraph::new(Line::from(status_right)).alignment(Alignment::Right);

    frame.render_widget(left_para, inner_chunks[0]);
    frame.render_widget(right_para, inner_chunks[1]);
}

/// Render connecting overlay with spinner
fn render_connecting_overlay(
    frame: &mut Frame,
    theme: &crate::tui::Theme,
    area: Rect,
    host_name: &str,
    start_time: Option<std::time::Instant>,
) {
    use ratatui::widgets::Clear;

    // Spinner frames for animation
    const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    // Calculate spinner frame based on elapsed time
    let frame_idx = start_time
        .map(|t| (t.elapsed().as_millis() / 80) as usize % SPINNER_FRAMES.len())
        .unwrap_or(0);
    let spinner = SPINNER_FRAMES[frame_idx];

    // Calculate elapsed time for display
    let elapsed = start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0);

    // Calculate overlay size and position (centered)
    let width = 50u16.min(area.width.saturating_sub(4));
    let height = 7u16;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let overlay_area = Rect::new(x, y, width, height);

    // Clear the area behind the overlay
    frame.render_widget(Clear, overlay_area);

    // Create overlay block with border
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" ", theme.title()),
            Span::styled(spinner, Style::default().fg(theme.accent_primary())),
            Span::styled(" Connecting ", theme.title()),
        ]))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_primary()))
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    // Content
    let content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Connecting to ", theme.text()),
            Span::styled(
                host_name,
                Style::default()
                    .fg(theme.accent_info())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("...", theme.text()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("Elapsed: {}s", elapsed),
            theme.text_dim(),
        )]),
        Line::from(vec![
            Span::styled("Press ", theme.text_dim()),
            Span::styled("Esc", theme.key_hint()),
            Span::styled(" to cancel", theme.text_dim()),
        ]),
    ];

    let paragraph = Paragraph::new(content).alignment(Alignment::Center);

    frame.render_widget(paragraph, inner);
}

/// Render host search overlay with input and filtered list
fn render_host_search_overlay(frame: &mut Frame, state: &RenderState, area: Rect) {
    use ratatui::widgets::Clear;

    let theme = &state.theme;

    // Calculate overlay size and position (centered)
    let max_results = 10u16;
    let width = 60u16.min(area.width.saturating_sub(4));
    let height = (5 + max_results).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let overlay_area = Rect::new(x, y, width, height);

    // Clear the area behind the overlay
    frame.render_widget(Clear, overlay_area);

    // Create overlay block with border
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" 󰍉 ", Style::default().fg(theme.accent_primary())),
            Span::styled("Search Hosts", theme.title()),
            Span::styled(" ", theme.title()),
        ]))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_primary()))
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    // Split inner area: input + results + hints
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input box
            Constraint::Min(1),    // Results list
            Constraint::Length(1), // Hints
        ])
        .split(inner);

    // Input box
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_info()))
        .title(Span::styled(" Type to filter ", theme.text_dim()))
        .style(Style::default().bg(theme.bg_panel()));

    let input_content = format!("{}_", state.host_search_query);
    let input = Paragraph::new(input_content)
        .style(Style::default().fg(theme.fg_bright()))
        .block(input_block);
    frame.render_widget(input, chunks[0]);

    // Results list - get host info for display
    let hosts = {
        let mut hosts_info: Vec<(usize, String, String)> = Vec::new();
        let mut idx = 0usize;
        for group in &state.config.groups {
            if group.expanded {
                for host in &group.hosts {
                    hosts_info.push((idx, host.name.clone(), host.connection_string()));
                    idx += 1;
                }
            }
        }
        for host in &state.config.hosts {
            hosts_info.push((idx, host.name.clone(), host.connection_string()));
            idx += 1;
        }
        hosts_info
    };

    // Build result list items
    let result_items: Vec<ListItem> = state
        .host_search_results
        .iter()
        .take(max_results as usize)
        .enumerate()
        .map(|(display_idx, &host_idx)| {
            let is_selected = display_idx == state.host_search_selected;

            if let Some((_, name, conn)) = hosts.get(host_idx) {
                let (prefix, name_style, conn_style) = if is_selected {
                    (
                        "▶ ",
                        Style::default()
                            .fg(theme.accent_primary())
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(theme.accent_info()),
                    )
                } else {
                    ("  ", theme.text_bright(), theme.text_dim())
                };

                ListItem::new(Line::from(vec![
                    Span::styled(
                        prefix,
                        if is_selected {
                            Style::default().fg(theme.accent_primary())
                        } else {
                            theme.text()
                        },
                    ),
                    Span::styled(format!("{:16}", name), name_style),
                    Span::styled(" │ ", theme.text_dim()),
                    Span::styled(conn.clone(), conn_style),
                ]))
            } else {
                ListItem::new(Line::from(""))
            }
        })
        .collect();

    if result_items.is_empty() {
        let empty_msg = if state.host_search_query.is_empty() {
            "No hosts configured"
        } else {
            "No matching hosts"
        };
        let empty = Paragraph::new(empty_msg)
            .style(theme.text_dim())
            .alignment(Alignment::Center);
        frame.render_widget(empty, chunks[1]);
    } else {
        let list = List::new(result_items);
        frame.render_widget(list, chunks[1]);
    }

    // Hints at bottom
    let hints = Line::from(vec![
        Span::styled("Enter", theme.key_hint()),
        Span::styled(":Select  ", theme.text_dim()),
        Span::styled("↑↓", theme.key_hint()),
        Span::styled(":Navigate  ", theme.text_dim()),
        Span::styled("Esc", theme.key_hint()),
        Span::styled(":Cancel", theme.text_dim()),
    ]);
    let hints_para = Paragraph::new(hints).alignment(Alignment::Center);
    frame.render_widget(hints_para, chunks[2]);
}

fn render_host_list_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;
    let icons = &state.icons;

    let title = Line::from(vec![
        Span::styled(format!(" {} ", icons.connections), theme.title()),
        Span::styled("Connections", theme.title()),
        Span::styled(" ", theme.title()),
    ]);

    // Host count badge in title
    let total_hosts = state.config.hosts.len()
        + state
            .config
            .groups
            .iter()
            .map(|g| g.hosts.len())
            .sum::<usize>();
    let host_count = format!(" [{}] ", total_hosts);

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .padding(Padding::new(2, 2, 1, 1)) // left, right, top, bottom
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner area: header + list
    let list_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Header
            Constraint::Min(1),    // List
        ])
        .split(inner);

    // Render header - matches column format in format_host_line_with_selection
    // Format: prefix(2) + status(2) + space(1) + auth(2) + space(1) + name(16) + " │ "(3) + host(28)
    let header = vec![
        Line::from(vec![
            Span::styled("  ", theme.text_dim()), // prefix (2)
            Span::styled("  ", theme.text_dim()), // status (2)
            Span::styled(" ", theme.text_dim()),  // space (1)
            Span::styled("  ", theme.text_dim()), // auth (2)
            Span::styled(
                "NAME           ",
                Style::default()
                    .fg(theme.accent_primary())
                    .add_modifier(Modifier::DIM),
            ), // name (16)
            Span::styled("  │ ", theme.text_dim()), // separator (3)
            Span::styled(
                "HOST",
                Style::default()
                    .fg(theme.accent_primary())
                    .add_modifier(Modifier::DIM),
            ), // host label
        ]),
        Line::from(vec![Span::styled(
            "  ─────────────────────────────────────────────────────────",
            Style::default()
                .fg(theme.accent_primary())
                .add_modifier(Modifier::DIM),
        )]),
    ];
    let header_para = Paragraph::new(header);
    frame.render_widget(header_para, list_chunks[0]);

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
            Line::from(vec![Span::styled(
                "  ┌─────────────────────────────────────┐",
                Style::default()
                    .fg(theme.accent_primary())
                    .add_modifier(Modifier::DIM),
            )]),
            Line::from(vec![
                Span::styled(
                    "  │  ",
                    Style::default()
                        .fg(theme.accent_primary())
                        .add_modifier(Modifier::DIM),
                ),
                Span::styled("  No connections configured", theme.text_dim()),
                Span::styled(
                    "      │",
                    Style::default()
                        .fg(theme.accent_primary())
                        .add_modifier(Modifier::DIM),
                ),
            ]),
            Line::from(vec![Span::styled(
                "  │                                     │",
                Style::default()
                    .fg(theme.accent_primary())
                    .add_modifier(Modifier::DIM),
            )]),
            Line::from(vec![
                Span::styled(
                    "  │  ",
                    Style::default()
                        .fg(theme.accent_primary())
                        .add_modifier(Modifier::DIM),
                ),
                Span::styled("Press ", theme.text_dim()),
                Span::styled("n", theme.key_hint()),
                Span::styled(" to add a new host", theme.text_dim()),
                Span::styled(
                    "       │",
                    Style::default()
                        .fg(theme.accent_primary())
                        .add_modifier(Modifier::DIM),
                ),
            ]),
            Line::from(vec![Span::styled(
                "  └─────────────────────────────────────┘",
                Style::default()
                    .fg(theme.accent_primary())
                    .add_modifier(Modifier::DIM),
            )]),
        ];
        let empty = Paragraph::new(empty_text);
        frame.render_widget(empty, list_chunks[1]);
    } else {
        let list = List::new(items);
        frame.render_widget(list, list_chunks[1]);
    }
}

/// Format a single host line with selection state
/// Uses fixed-width columns for proper alignment
fn format_host_line_with_selection(
    host: &crate::config::HostConfig,
    theme: &crate::tui::Theme,
    icons: &crate::tui::Icons,
    is_selected: bool,
) -> Line<'static> {
    // Column widths
    const NAME_WIDTH: usize = 16;
    const HOST_WIDTH: usize = 28;

    // Status indicator
    let status_icon = if is_selected { "●" } else { "○" };

    // Auth method icon
    let auth_icon = match &host.auth {
        crate::config::AuthMethod::Password => icons.password,
        crate::config::AuthMethod::KeyFile { .. } => icons.key_file,
        crate::config::AuthMethod::Agent => icons.agent,
        crate::config::AuthMethod::Certificate { .. } => icons.certificate,
    };

    // Truncate and pad name to fixed width
    let name = if host.name.len() > NAME_WIDTH {
        format!("{}…", &host.name[..NAME_WIDTH - 1])
    } else {
        format!("{:width$}", host.name, width = NAME_WIDTH)
    };

    // Truncate and pad host connection string to fixed width
    let conn_str = host.connection_string();
    let host_display = if conn_str.len() > HOST_WIDTH {
        format!("{}…", &conn_str[..HOST_WIDTH - 1])
    } else {
        format!("{:width$}", conn_str, width = HOST_WIDTH)
    };

    // Styles for selected vs unselected - include bg color to prevent black blocks
    let bg = theme.bg_panel();
    let (prefix, status_style, name_style, host_style) = if is_selected {
        (
            "▶ ",
            Style::default()
                .fg(theme.accent_success())
                .bg(bg)
                .add_modifier(Modifier::SLOW_BLINK),
            Style::default()
                .fg(theme.accent_primary())
                .bg(bg)
                .add_modifier(Modifier::BOLD),
            Style::default().fg(theme.accent_info()).bg(bg),
        )
    } else {
        (
            "  ",
            theme.text_dim().bg(bg),
            theme.text_bright().bg(bg),
            theme.text_dim().bg(bg),
        )
    };

    Line::from(vec![
        Span::styled(
            prefix.to_string(),
            if is_selected {
                Style::default()
                    .fg(theme.accent_primary())
                    .bg(bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.text().bg(bg)
            },
        ),
        Span::styled(format!("{} ", status_icon), status_style),
        Span::styled(
            format!("{} ", auth_icon),
            Style::default().fg(theme.accent_info()).bg(bg),
        ),
        Span::styled(name, name_style),
        Span::styled(" │ ", theme.text_dim().bg(bg)),
        Span::styled(host_display, host_style),
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
        Line::from(vec![Span::styled(
            "󰋼 Quick Start",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.fg_bright()),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("Enter", theme.key_hint()),
            Span::styled(" Connect to host", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("n", theme.key_hint()),
            Span::styled("     New connection", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("e", theme.key_hint()),
            Span::styled("     Edit selected", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("d", theme.key_hint()),
            Span::styled("     Delete selected", theme.text()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "󰌌 Navigation",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.fg_bright()),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("j/↓", theme.key_hint()),
            Span::styled("   Move down", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("k/↑", theme.key_hint()),
            Span::styled("   Move up", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("/", theme.key_hint()),
            Span::styled("     Search hosts", theme.text()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "󰌑 Views",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.fg_bright()),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("f", theme.key_hint()),
            Span::styled("     SFTP Browser", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("t", theme.key_hint()),
            Span::styled("     Tunnels", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("s", theme.key_hint()),
            Span::styled("     Settings", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("?", theme.key_hint()),
            Span::styled("     Help", theme.text()),
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
            Constraint::Percentage(60), // Host list
            Constraint::Percentage(40), // Details panel
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
            Span::styled(
                &group.name,
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(theme.fg_bright()),
            ),
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
            Span::styled(
                "Ungrouped",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(theme.fg_bright()),
            ),
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
            Line::from(vec![Span::styled(
                "  No connections configured",
                theme.text_dim(),
            )]),
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
        Line::from(vec![Span::styled(
            "󰋼 Quick Start",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.fg_bright()),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("Enter", theme.key_hint()),
            Span::styled(" Connect to host", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("n", theme.key_hint()),
            Span::styled("     New connection", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("e", theme.key_hint()),
            Span::styled("     Edit selected", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("d", theme.key_hint()),
            Span::styled("     Delete selected", theme.text()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "󰌌 Navigation",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.fg_bright()),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("j/↓", theme.key_hint()),
            Span::styled("   Move down", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("k/↑", theme.key_hint()),
            Span::styled("   Move up", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("/", theme.key_hint()),
            Span::styled("     Search hosts", theme.text()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "󰌑 Views",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.fg_bright()),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("f", theme.key_hint()),
            Span::styled("     SFTP Browser", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("t", theme.key_hint()),
            Span::styled("     Tunnels", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("K", theme.key_hint()),
            Span::styled("     Key Manager", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("s", theme.key_hint()),
            Span::styled("     Settings", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("?", theme.key_hint()),
            Span::styled("     Help", theme.text()),
        ]),
    ];

    let paragraph = Paragraph::new(content);
    frame.render_widget(paragraph, inner);
}
