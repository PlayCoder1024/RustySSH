//! SSH key management view

use crate::app::{App, RenderState};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Table};

/// Render keys view with RenderState
pub fn render_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;

    let title = Line::from(vec![
        Span::styled(" 󰌋 ", theme.title()),
        Span::styled("SSH Keys", theme.title()),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .padding(Padding::uniform(1))
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.ssh_keys.is_empty() {
        let empty_text = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  No SSH keys found in ~/.ssh/",
                theme.text_dim(),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Press ", theme.text_dim()),
                Span::styled("n", theme.key_hint()),
                Span::styled(" to generate a new key", theme.text_dim()),
            ]),
        ];
        frame.render_widget(Paragraph::new(empty_text), inner);
    } else {
        // Key table header
        let header = Row::new(vec![
            Cell::from("Type").style(theme.text_dim()),
            Cell::from("Name").style(theme.text_dim()),
            Cell::from("Fingerprint").style(theme.text_dim()),
            Cell::from("Comment").style(theme.text_dim()),
            Cell::from("Encrypted").style(theme.text_dim()),
        ]);

        let rows: Vec<Row> = state.ssh_keys.iter().enumerate().map(|(i, key)| {
            let is_selected = i == state.settings_item; // Reusing settings_item for selection index if in settings view
            // Note: This relies on settings_item being the index. If we are in View::Keys, we need a different index in RenderState?
            // The plan is to put this in Settings. In Settings, settings_item tracks the selected item in the list.
            // If we are just listing keys, we might need a separate 'selected_key_index'. 
            // BUT, for now let's assume valid mapping or just not highlight if not applicable.
            // Actually, in Settings view, `settings_item` tracks the index within the category. 
            // If we are in the "Keys" category, `settings_item` will be the key index.
            
            create_key_row_snapshot(key, theme, is_selected)
        }).collect();

        let widths = [
            Constraint::Length(10), // Type
            Constraint::Length(15), // Name
            Constraint::Length(45), // Fingerprint
            Constraint::Min(15),    // Comment
            Constraint::Length(10), // Encrypted
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .highlight_style(theme.selected());

        // Split for help text at bottom
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(3)])
            .split(inner);

        frame.render_widget(table, chunks[0]);

        // Help text
        let help = Line::from(vec![
            Span::styled("n", theme.key_hint()),
            Span::styled(":Generate  ", theme.text_dim()),
            Span::styled("d", theme.key_hint()),
            Span::styled(":Delete  ", theme.text_dim()),
             Span::styled("r", theme.key_hint()),
            Span::styled(":Refresh", theme.text_dim()),
        ]);
        frame.render_widget(Paragraph::new(help), chunks[1]);
    }
}

use crate::app::KeyInfoSnapshot;

fn create_key_row_snapshot<'a>(
    key: &'a KeyInfoSnapshot,
    theme: &crate::tui::Theme,
    is_selected: bool,
) -> Row<'a> {
    let base_style = if is_selected {
        theme.selected()
    } else {
        theme.text()
    };

    let type_style = Style::default().fg(match key.key_type.as_str() {
        "ed25519" => theme.accent_success(),
        "rsa" => theme.accent_primary(),
        "ecdsa" => theme.accent_info(),
        _ => theme.fg_dim(),
    });

    let encrypted_text = if key.encrypted { "🔒 Yes" } else { "🔓 No" };
    let encrypted_style = Style::default().fg(if key.encrypted {
        theme.accent_warning()
    } else {
        theme.fg_dim()
    });

    Row::new(vec![
        Cell::from(key.key_type.clone()).style(type_style),
        Cell::from(key.name.clone()).style(base_style),
        Cell::from(key.fingerprint.clone()).style(theme.text_dim()),
        Cell::from(key.comment.clone()).style(theme.text()),
        Cell::from(encrypted_text).style(encrypted_style),
    ])
}

/// Render the keys view
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let title = Line::from(vec![
        Span::styled(" 󰌋 ", theme.title()),
        Span::styled("SSH Keys", theme.title()),
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

    // Key table header
    let header = Row::new(vec![
        Cell::from("Type").style(theme.text_dim()),
        Cell::from("Name").style(theme.text_dim()),
        Cell::from("Fingerprint").style(theme.text_dim()),
        Cell::from("Comment").style(theme.text_dim()),
        Cell::from("Encrypted").style(theme.text_dim()),
    ]);

    // Sample key data (would come from KeyManager)
    let rows = vec![
        create_key_row(
            "ED25519",
            "id_ed25519",
            "SHA256:abc123...",
            "user@hostname",
            false,
            theme,
            true,
        ),
        create_key_row(
            "RSA",
            "id_rsa",
            "SHA256:def456...",
            "backup key",
            true,
            theme,
            false,
        ),
        create_key_row(
            "ED25519",
            "github",
            "SHA256:ghi789...",
            "github auth",
            false,
            theme,
            false,
        ),
    ];

    let widths = [
        Constraint::Length(10), // Type
        Constraint::Length(15), // Name
        Constraint::Length(20), // Fingerprint
        Constraint::Min(15),    // Comment
        Constraint::Length(10), // Encrypted
    ];

    if rows.is_empty() {
        let empty_text = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  No SSH keys found in ~/.ssh/",
                theme.text_dim(),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Press ", theme.text_dim()),
                Span::styled("n", theme.key_hint()),
                Span::styled(" to generate a new key", theme.text_dim()),
            ]),
            Line::from(vec![
                Span::styled("  Press ", theme.text_dim()),
                Span::styled("i", theme.key_hint()),
                Span::styled(" to import an existing key", theme.text_dim()),
            ]),
        ];
        let empty = Paragraph::new(empty_text);
        frame.render_widget(empty, inner);
    } else {
        // Split area for table and details
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(inner);

        let table = Table::new(rows, widths)
            .header(header)
            .highlight_style(theme.selected());

        frame.render_widget(table, chunks[0]);

        // Key details / actions
        render_key_details(frame, app, chunks[1]);
    }
}

/// Create a key row
fn create_key_row<'a>(
    key_type: &'a str,
    name: &'a str,
    fingerprint: &'a str,
    comment: &'a str,
    encrypted: bool,
    theme: &crate::tui::Theme,
    is_selected: bool,
) -> Row<'a> {
    let base_style = if is_selected {
        theme.selected()
    } else {
        theme.text()
    };

    let type_style = Style::default().fg(match key_type {
        "ED25519" => theme.accent_success(),
        "RSA" => theme.accent_primary(),
        "ECDSA" => theme.accent_info(),
        _ => theme.fg_dim(),
    });

    let encrypted_text = if encrypted { "🔒 Yes" } else { "🔓 No" };
    let encrypted_style = Style::default().fg(if encrypted {
        theme.accent_warning()
    } else {
        theme.fg_dim()
    });

    Row::new(vec![
        Cell::from(key_type).style(type_style),
        Cell::from(name).style(base_style),
        Cell::from(fingerprint).style(theme.text_dim()),
        Cell::from(comment).style(theme.text()),
        Cell::from(encrypted_text).style(encrypted_style),
    ])
}

/// Render key details panel
fn render_key_details(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let block = Block::default()
        .title(" Key Details ")
        .borders(Borders::TOP)
        .border_style(theme.border_normal())
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let details = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Actions: ",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.fg_bright()),
        )]),
        Line::from(vec![
            Span::styled("  ", theme.text()),
            Span::styled("v", theme.key_hint()),
            Span::styled(" View public key  ", theme.text()),
            Span::styled("c", theme.key_hint()),
            Span::styled(" Copy to clipboard  ", theme.text()),
            Span::styled("d", theme.key_hint()),
            Span::styled(" Delete  ", theme.text()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Generate: ",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(theme.fg_bright()),
        )]),
        Line::from(vec![
            Span::styled("  ", theme.text()),
            Span::styled("n", theme.key_hint()),
            Span::styled(" New ED25519 (recommended)  ", theme.text()),
            Span::styled("N", theme.key_hint()),
            Span::styled(" New RSA-4096", theme.text()),
        ]),
    ];

    let paragraph = Paragraph::new(details);
    frame.render_widget(paragraph, inner);
}
