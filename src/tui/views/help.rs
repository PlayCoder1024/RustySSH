//! Help view

use crate::app::{App, RenderState};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

/// Render help view with RenderState
pub fn render_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;

    let title = Line::from(vec![
        Span::styled(" 󰋖 ", theme.title()),
        Span::styled("Help", theme.title()),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .padding(Padding::uniform(2))
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let help_text = vec![
        Line::from(vec![
            Span::styled(
                "RustySSH",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(theme.accent_primary()),
            ),
            Span::styled(" - Terminal SSH Connection Manager", theme.text()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            " Global Shortcuts",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.fg_bright()),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("   Ctrl+Q      ", theme.key_hint()),
            Span::styled("Quit application", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("   ?           ", theme.key_hint()),
            Span::styled("Show/hide help", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("   Esc         ", theme.key_hint()),
            Span::styled("Go back / Cancel", theme.text()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Press ", theme.text_dim()),
            Span::styled("Esc", theme.key_hint()),
            Span::styled(" to close this help", theme.text_dim()),
        ]),
    ];

    frame.render_widget(Paragraph::new(help_text), inner);
}

/// Render the help view
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let title = Line::from(vec![
        Span::styled(" 󰋖 ", theme.title()),
        Span::styled("Help", theme.title()),
        Span::styled(" ", theme.title()),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .padding(Padding::uniform(2))
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let help_text = vec![
        Line::from(vec![
            Span::styled(
                "RustySSH",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(theme.accent_primary()),
            ),
            Span::styled(" - Terminal SSH Connection Manager", theme.text()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            " Global Shortcuts",
            Style::default()
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                .fg(theme.fg_bright()),
        )]),
        Line::from(""),
        create_help_line("Ctrl+Q / Ctrl+C", "Quit application", theme),
        create_help_line("?", "Show/hide help", theme),
        create_help_line("Esc", "Go back / Cancel", theme),
        Line::from(""),
        Line::from(vec![Span::styled(
            " Navigation",
            Style::default()
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                .fg(theme.fg_bright()),
        )]),
        Line::from(""),
        create_help_line("↑/k, ↓/j", "Move up/down", theme),
        create_help_line("PgUp, PgDown", "Page up/down", theme),
        create_help_line("Home/g, End/G", "Go to start/end", theme),
        create_help_line("Tab", "Switch pane (SFTP)", theme),
        create_help_line("Enter", "Select / Confirm", theme),
        Line::from(""),
        Line::from(vec![Span::styled(
            " Views",
            Style::default()
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                .fg(theme.fg_bright()),
        )]),
        Line::from(""),
        create_help_line("c", "Connections (home)", theme),
        create_help_line("t", "SSH Tunnels", theme),
        create_help_line("f", "SFTP File Browser", theme),
        create_help_line("k", "SSH Key Manager", theme),
        create_help_line("s", "Settings", theme),
        Line::from(""),
        Line::from(vec![Span::styled(
            " Connection Actions",
            Style::default()
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                .fg(theme.fg_bright()),
        )]),
        Line::from(""),
        create_help_line("n", "New connection", theme),
        create_help_line("e", "Edit connection", theme),
        create_help_line("d", "Delete connection", theme),
        create_help_line("Enter", "Connect to selected", theme),
        Line::from(""),
        Line::from(vec![Span::styled(
            " Session Shortcuts",
            Style::default()
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                .fg(theme.fg_bright()),
        )]),
        Line::from(""),
        create_help_line("Shift+Esc", "Return to connections", theme),
        create_help_line("Ctrl+Shift+T", "New session tab", theme),
        create_help_line("Ctrl+Shift+W", "Close current tab", theme),
        create_help_line("Ctrl+Tab", "Next session tab", theme),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Press ", theme.text_dim()),
            Span::styled("Esc", theme.key_hint()),
            Span::styled(" or ", theme.text_dim()),
            Span::styled("?", theme.key_hint()),
            Span::styled(" to close this help", theme.text_dim()),
        ]),
    ];

    let paragraph = Paragraph::new(help_text).wrap(ratatui::widgets::Wrap { trim: false });

    frame.render_widget(paragraph, inner);
}

/// Create a help line with key and description
fn create_help_line<'a>(key: &'a str, desc: &'a str, theme: &crate::tui::Theme) -> Line<'a> {
    Line::from(vec![
        Span::styled("   ", theme.text()),
        Span::styled(format!("{:20}", key), theme.key_hint()),
        Span::styled(desc, theme.text()),
    ])
}
