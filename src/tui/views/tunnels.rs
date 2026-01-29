//! SSH tunnels view

use crate::app::{App, RenderState};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Gauge, Padding, Paragraph, Row, Table};

/// Render tunnels view with RenderState
pub fn render_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;

    let title = Line::from(vec![
        Span::styled(" 󰛳 ", theme.title()),
        Span::styled("SSH Tunnels", theme.title()),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .padding(Padding::uniform(1))
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let empty_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  No tunnels configured",
            theme.text_dim(),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Press ", theme.text_dim()),
            Span::styled("n", theme.key_hint()),
            Span::styled(" to create a new tunnel", theme.text_dim()),
        ]),
    ];
    frame.render_widget(Paragraph::new(empty_text), inner);
}

/// Render the tunnels view
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

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

    // Tunnel table
    let header = Row::new(vec![
        Cell::from("Status").style(theme.text_dim()),
        Cell::from("Name").style(theme.text_dim()),
        Cell::from("Type").style(theme.text_dim()),
        Cell::from("Configuration").style(theme.text_dim()),
        Cell::from("↑ Sent").style(theme.text_dim()),
        Cell::from("↓ Recv").style(theme.text_dim()),
        Cell::from("Conns").style(theme.text_dim()),
    ]);

    // Sample tunnel data
    let rows = vec![
        create_tunnel_row(
            "●",
            "DB Forward",
            "Local",
            "localhost:3306 → db.prod:3306",
            "12.4 MB",
            "45.2 MB",
            "3",
            theme,
            true,
        ),
        create_tunnel_row(
            "●",
            "Web Proxy",
            "Dynamic",
            "localhost:1080 (SOCKS5)",
            "1.2 GB",
            "3.4 GB",
            "12",
            theme,
            false,
        ),
        create_tunnel_row(
            "○",
            "Remote SSH",
            "Remote",
            "remote:2222 → localhost:22",
            "--",
            "--",
            "0",
            theme,
            false,
        ),
    ];

    let widths = [
        Constraint::Length(6),  // Status
        Constraint::Length(12), // Name
        Constraint::Length(8),  // Type
        Constraint::Min(30),    // Config
        Constraint::Length(10), // Sent
        Constraint::Length(10), // Recv
        Constraint::Length(6),  // Conns
    ];

    // Check if we have any tunnels
    if rows.is_empty() {
        let empty_text = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  No tunnels configured",
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
        let empty = Paragraph::new(empty_text);
        frame.render_widget(empty, inner);
    } else {
        let table = Table::new(rows, widths)
            .header(header)
            .highlight_style(theme.selected())
            .highlight_style(theme.selected());

        frame.render_widget(table, inner);
    }
}

/// Create a tunnel row
fn create_tunnel_row<'a>(
    status: &'a str,
    name: &'a str,
    tunnel_type: &'a str,
    config: &'a str,
    sent: &'a str,
    recv: &'a str,
    conns: &'a str,
    theme: &crate::tui::Theme,
    is_active: bool,
) -> Row<'a> {
    let status_style = if status == "●" {
        theme.success()
    } else {
        theme.text_dim()
    };

    let type_style = Style::default().fg(match tunnel_type {
        "Local" => theme.accent_primary(),
        "Remote" => theme.accent_secondary(),
        "Dynamic" => theme.accent_info(),
        _ => return Row::new(Vec::<Cell>::new()),
    });

    let base_style = if is_active {
        theme.selected()
    } else {
        theme.text()
    };

    Row::new(vec![
        Cell::from(status).style(status_style),
        Cell::from(name).style(base_style),
        Cell::from(tunnel_type).style(type_style),
        Cell::from(config).style(theme.text_dim()),
        Cell::from(sent).style(theme.accent_success()),
        Cell::from(recv).style(theme.accent_info()),
        Cell::from(conns).style(theme.text()),
    ])
}
