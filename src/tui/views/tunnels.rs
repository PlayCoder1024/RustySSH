//! SSH tunnels view

use crate::app::{App, RenderState};
use crate::config::TunnelConfig;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Clear, Padding, Paragraph, Row, Table};

/// Render tunnels view with RenderState
pub fn render_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    render_tunnels_view(
        frame,
        area,
        &state.theme,
        &state.config,
        state.tunnel_selected_index,
        state.tunnel_edit_visible,
        state.tunnel_edit_is_new,
        state.tunnel_edit_draft.as_ref(),
        state.tunnel_edit_field_index,
        state.tunnel_editing,
        &state.tunnel_temp_buffer,
    );
}

/// Render the tunnels view (legacy)
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    render_tunnels_view(
        frame,
        area,
        &app.theme,
        &app.config,
        app.tunnel_selected_index,
        app.tunnel_edit_visible,
        app.tunnel_edit_is_new,
        app.tunnel_edit_draft.as_ref(),
        app.tunnel_edit_field_index,
        app.tunnel_editing,
        &app.tunnel_temp_buffer,
    );
}

fn render_tunnels_view(
    frame: &mut Frame,
    area: Rect,
    theme: &crate::tui::Theme,
    config: &crate::config::Config,
    selected_index: usize,
    edit_visible: bool,
    edit_is_new: bool,
    edit_draft: Option<&TunnelConfig>,
    edit_field_index: usize,
    edit_is_editing: bool,
    edit_buffer: &str,
) {
    let title = Line::from(vec![
        Span::styled(" 󰛳 ", theme.title()),
        Span::styled("SSH Tunnels", theme.title()),
        Span::styled(" ", theme.title()),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .padding(Padding::uniform(1))
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if config.tunnels.is_empty() {
        let empty_text = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  No tunnels defined",
                theme.text_dim(),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Press ", theme.text_dim()),
                Span::styled("n", theme.key_hint()),
                Span::styled(" to create a new tunnel", theme.text_dim()),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Tunnel Types:",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(theme.fg_bright()),
            )]),
            Line::from(vec![
                Span::styled("  • ", theme.accent_primary()),
                Span::styled("Local (-L): ", theme.text_bright()),
                Span::styled("Forward local port to remote", theme.text_dim()),
            ]),
            Line::from(vec![
                Span::styled("  • ", theme.accent_secondary()),
                Span::styled("Remote (-R): ", theme.text_bright()),
                Span::styled("Forward remote port to local", theme.text_dim()),
            ]),
            Line::from(vec![
                Span::styled("  • ", theme.accent_info()),
                Span::styled("Dynamic (-D): ", theme.text_bright()),
                Span::styled("SOCKS5 proxy", theme.text_dim()),
            ]),
        ];
        frame.render_widget(Paragraph::new(empty_text), inner);
    } else {
        render_tunnel_table(frame, inner, theme, config, selected_index);
    }

    if edit_visible {
        render_tunnel_edit_overlay(
            frame,
            area,
            theme,
            config,
            selected_index,
            edit_is_new,
            edit_draft,
            edit_field_index,
            edit_is_editing,
            edit_buffer,
        );
    }
}

fn render_tunnel_table(
    frame: &mut Frame,
    area: Rect,
    theme: &crate::tui::Theme,
    config: &crate::config::Config,
    selected_index: usize,
) {
    let header = Row::new(vec![
        Cell::from("Name").style(theme.text_dim()),
        Cell::from("Type").style(theme.text_dim()),
        Cell::from("Configuration").style(theme.text_dim()),
        Cell::from("Auto").style(theme.text_dim()),
        Cell::from("Hosts").style(theme.text_dim()),
    ]);

    let rows: Vec<Row> = config
        .tunnels
        .iter()
        .enumerate()
        .map(|(idx, tunnel)| {
            let is_selected = idx == selected_index;
            create_tunnel_row(
                tunnel,
                count_hosts_using_tunnel(config, tunnel.name()),
                theme,
                is_selected,
            )
        })
        .collect();

    let widths = [
        Constraint::Length(18), // Name
        Constraint::Length(8),  // Type
        Constraint::Min(30),    // Config
        Constraint::Length(6),  // Auto
        Constraint::Length(6),  // Hosts
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .highlight_style(theme.selected());

    frame.render_widget(table, area);
}

fn create_tunnel_row<'a>(
    tunnel: &'a TunnelConfig,
    host_count: usize,
    theme: &crate::tui::Theme,
    is_selected: bool,
) -> Row<'a> {
    let type_style = Style::default().fg(match tunnel {
        TunnelConfig::Local { .. } => theme.accent_primary(),
        TunnelConfig::Remote { .. } => theme.accent_secondary(),
        TunnelConfig::Dynamic { .. } => theme.accent_info(),
    });

    let base_style = if is_selected {
        theme.selected()
    } else {
        theme.text()
    };

    let auto_display = if tunnel.auto_start() { "Yes" } else { "No" };

    Row::new(vec![
        Cell::from(tunnel.name()).style(base_style),
        Cell::from(tunnel.type_label()).style(type_style),
        Cell::from(tunnel.description()).style(theme.text_dim()),
        Cell::from(auto_display).style(theme.text_bright()),
        Cell::from(host_count.to_string()).style(theme.text()),
    ])
    .style(if is_selected {
        theme.selected()
    } else {
        Style::default()
    })
}

fn count_hosts_using_tunnel(config: &crate::config::Config, name: &str) -> usize {
    let mut count = 0;
    for group in &config.groups {
        for host in &group.hosts {
            if host.tunnels.iter().any(|t| t.name() == name) {
                count += 1;
            }
        }
    }
    for host in &config.hosts {
        if host.tunnels.iter().any(|t| t.name() == name) {
            count += 1;
        }
    }
    count
}

fn render_tunnel_edit_overlay(
    frame: &mut Frame,
    area: Rect,
    theme: &crate::tui::Theme,
    config: &crate::config::Config,
    selected_index: usize,
    is_new: bool,
    draft: Option<&TunnelConfig>,
    field_index: usize,
    is_editing: bool,
    edit_buffer: &str,
) {
    let tunnel = draft.or_else(|| config.tunnels.get(selected_index));
    let Some(tunnel) = tunnel else {
        return;
    };

    let title_text = if is_new { "New Tunnel" } else { "Edit Tunnel" };

    let fields = tunnel_fields_for_display(tunnel);
    let height = (fields.len() as u16 + 6).min(area.height.saturating_sub(2));
    let width = 70u16.min(area.width.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let overlay_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, overlay_area);

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" 󰛳 ", Style::default().fg(theme.accent_primary())),
            Span::styled(title_text, theme.title()),
            Span::styled(" ", theme.title()),
        ]))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let mut lines = Vec::new();
    lines.push(Line::from(""));

    for (i, (label, value)) in fields.iter().enumerate() {
        let is_selected = i == field_index;
        let is_active_edit = is_selected && is_editing;
        let row_bg = if is_selected {
            theme.bg_selected()
        } else {
            theme.bg_panel()
        };

        let value_text: &str = if is_active_edit { edit_buffer } else { value };

        let label_style = if is_selected {
            Style::default()
                .fg(theme.accent_primary())
                .bg(row_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg_dim()).bg(row_bg)
        };

        let value_style = if is_selected {
            Style::default().fg(theme.fg_bright()).bg(row_bg)
        } else {
            theme.text().bg(row_bg)
        };

        let cursor_style = Style::default()
            .fg(row_bg)
            .bg(theme.fg_bright())
            .add_modifier(Modifier::SLOW_BLINK);
        let mut line_spans = Vec::new();
        line_spans.push(Span::styled(format!("{:14}", label), label_style));
        line_spans.push(Span::styled(" │ ", theme.text_dim().bg(row_bg)));
        if is_active_edit {
            line_spans.push(Span::styled(value_text.to_string(), value_style));
            line_spans.push(Span::styled(" ", cursor_style));
        } else {
            line_spans.push(Span::styled(value_text.to_string(), value_style));
        }
        lines.push(Line::from(line_spans));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, chunks[0]);

    let hints = if is_editing {
        Line::from(vec![
            Span::styled("Enter", theme.key_hint()),
            Span::styled(":Save  ", theme.text_dim()),
            Span::styled("Esc", theme.key_hint()),
            Span::styled(":Cancel  ", theme.text_dim()),
            Span::styled("Backspace", theme.key_hint()),
            Span::styled(":Delete", theme.text_dim()),
        ])
    } else {
        Line::from(vec![
            Span::styled("Enter", theme.key_hint()),
            Span::styled(":Edit/Toggle  ", theme.text_dim()),
            Span::styled("↑↓", theme.key_hint()),
            Span::styled(":Navigate  ", theme.text_dim()),
            Span::styled("Esc", theme.key_hint()),
            Span::styled(":Close", theme.text_dim()),
        ])
    };

    frame.render_widget(Paragraph::new(hints).alignment(Alignment::Center), chunks[1]);
}

fn tunnel_fields_for_display(tunnel: &TunnelConfig) -> Vec<(String, String)> {
    let mut fields = vec![
        ("Name".to_string(), tunnel.name().to_string()),
        ("Type".to_string(), tunnel.type_label().to_string()),
        (
            "Auto-start".to_string(),
            if tunnel.auto_start() {
                "Yes".to_string()
            } else {
                "No".to_string()
            },
        ),
    ];

    match tunnel {
        TunnelConfig::Local {
            bind_addr,
            bind_port,
            remote_host,
            remote_port,
            ..
        } => {
            fields.push(("Bind Addr".to_string(), bind_addr.clone()));
            fields.push(("Bind Port".to_string(), bind_port.to_string()));
            fields.push(("Remote Host".to_string(), remote_host.clone()));
            fields.push(("Remote Port".to_string(), remote_port.to_string()));
        }
        TunnelConfig::Remote {
            remote_addr,
            remote_port,
            local_host,
            local_port,
            ..
        } => {
            fields.push(("Remote Addr".to_string(), remote_addr.clone()));
            fields.push(("Remote Port".to_string(), remote_port.to_string()));
            fields.push(("Local Host".to_string(), local_host.clone()));
            fields.push(("Local Port".to_string(), local_port.to_string()));
        }
        TunnelConfig::Dynamic {
            bind_addr,
            bind_port,
            ..
        } => {
            fields.push(("Bind Addr".to_string(), bind_addr.clone()));
            fields.push(("Bind Port".to_string(), bind_port.to_string()));
        }
    }

    fields
}
