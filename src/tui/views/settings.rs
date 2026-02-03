//! Settings view with interactive controls

use crate::app::RenderState;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};

/// Categories and their items

const CATEGORIES: &[(&str, &str)] = &[
    ("󰔎 ", "Appearance"),
    ("󰣀 ", "SSH"),
    ("󰌋 ", "Keys"),
    ("󰈙 ", "Logging"),
    ("󰌑 ", "Keymap"),
    ("󰋜 ", "About"),
];

/// Render settings view with RenderState
pub fn render_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;

    let title = Line::from(vec![
        Span::styled(" 󰒓 ", theme.title()),
        Span::styled("Settings", theme.title()),
        Span::styled(" ", theme.title()),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .padding(Padding::uniform(1))
        .style(Style::default().bg(theme.bg_main()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(18), // Category list
            Constraint::Min(40),    // Settings content
        ])
        .split(inner);

    // Render categories
    render_categories(frame, state, chunks[0]);

    // Render separator
    let separator_area = Rect::new(
        chunks[0].x + chunks[0].width,
        chunks[0].y,
        1,
        chunks[0].height,
    );
    let separator = Block::default()
        .borders(Borders::LEFT)
        .border_style(theme.border_normal());
    frame.render_widget(separator, separator_area);

    // Render content
    render_content(frame, state, chunks[1]);

    // Render keyboard hints
    render_hints(frame, state, area);

    // Render dropdown overlay if open
    if state.settings_dropdown_open {
        render_dropdown(frame, state, chunks[1]);
    }
}

/// Render category list
fn render_categories(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;

    let mut y = area.y;
    for (i, (icon, name)) in CATEGORIES.iter().enumerate() {
        let is_selected = i == state.settings_category;

        let style = if is_selected {
            Style::default()
                .bg(theme.bg_selected())
                .fg(theme.fg_bright())
        } else {
            Style::default().fg(theme.fg_main())
        };

        let icon_style = if is_selected {
            Style::default()
                .bg(theme.bg_selected())
                .fg(theme.accent_primary())
        } else {
            Style::default().fg(theme.accent_info())
        };

        let line = Line::from(vec![
            Span::raw(" "),
            Span::styled(*icon, icon_style),
            Span::styled(*name, style),
        ]);

        let item_area = Rect::new(area.x, y, area.width, 1);
        frame.render_widget(
            Paragraph::new(line).style(if is_selected {
                Style::default().bg(theme.bg_selected())
            } else {
                Style::default()
            }),
            item_area,
        );

        y += 1;
    }
}

/// Render settings content for selected category
fn render_content(frame: &mut Frame, state: &RenderState, area: Rect) {
    let _theme = &state.theme;
    let content_area = Rect::new(
        area.x + 2,
        area.y,
        area.width.saturating_sub(2),
        area.height,
    );

    match state.settings_category {
        0 => render_appearance_settings(frame, state, content_area),
        1 => render_ssh_settings(frame, state, content_area),
        2 => crate::tui::views::keys::render_state(frame, state, content_area),
        3 => render_logging_settings(frame, state, content_area),
        4 => render_keymap_settings(frame, state, content_area),
        5 => render_about_settings(frame, state, content_area),
        _ => {}
    }
}

/// Render about settings
fn render_about_settings(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;

    // Header
    let header = Line::from(vec![Span::styled(
        "Author: PlayCoder",
        Style::default()
            .fg(theme.accent_primary())
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )]);
    frame.render_widget(
        Paragraph::new(header),
        Rect::new(area.x, area.y, area.width, 1),
    );

    let email = "bGl1amlhbnlvdXNoZW5nQGhvdG1haWwuY29tCg==";

    // Render email below ASCII art
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Contact me: ", theme.text_dim()),
            Span::styled(email, Style::default().fg(theme.accent_success())),
        ])),
        Rect::new(area.x, area.y + 1, area.width, 1),
    );
}

/// Toggle switch widget (visual representation)
fn toggle_switch(enabled: bool, theme: &crate::tui::Theme) -> Line<'static> {
    if enabled {
        Line::from(vec![
            Span::styled("󰔡 ", Style::default().fg(theme.accent_success())),
            Span::styled(
                "ON ",
                Style::default()
                    .fg(theme.accent_success())
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled("󰨙 ", Style::default().fg(theme.fg_dim())),
            Span::styled("OFF", Style::default().fg(theme.fg_dim())),
        ])
    }
}

/// Dropdown value display
fn dropdown_value(value: &str, theme: &crate::tui::Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            value.to_string(),
            Style::default().fg(theme.accent_primary()),
        ),
        Span::styled(" ▼", Style::default().fg(theme.fg_dim())),
    ])
}

/// Numeric value display
fn numeric_value(
    value: impl std::fmt::Display,
    unit: &str,
    theme: &crate::tui::Theme,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{}", value),
            Style::default().fg(theme.accent_info()),
        ),
        Span::styled(format!(" {}", unit), Style::default().fg(theme.fg_dim())),
    ])
}

/// Render a setting row
fn render_setting_row(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value_line: Line<'static>,
    is_selected: bool,
    theme: &crate::tui::Theme,
) {
    let bg = if is_selected {
        theme.bg_highlight()
    } else {
        theme.bg_main()
    };
    let label_style = if is_selected {
        Style::default().fg(theme.fg_bright()).bg(bg)
    } else {
        Style::default().fg(theme.fg_main()).bg(bg)
    };

    // Selection indicator
    let indicator = if is_selected { "▶ " } else { "  " };

    let row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(20)])
        .split(area);

    // Clear background for entire row
    frame.render_widget(Clear, area);
    frame.render_widget(Block::default().style(Style::default().bg(bg)), area);

    // Label
    let label_line = Line::from(vec![
        Span::styled(
            indicator,
            if is_selected {
                Style::default().fg(theme.accent_primary()).bg(bg)
            } else {
                Style::default().fg(theme.fg_dim()).bg(bg)
            },
        ),
        Span::styled(label, label_style),
    ]);
    frame.render_widget(Paragraph::new(label_line), row[0]);

    // Value with background
    let styled_value: Line<'static> = Line::from(
        value_line
            .spans
            .into_iter()
            .map(|span| Span::styled(span.content, span.style.bg(bg)))
            .collect::<Vec<_>>(),
    );
    frame.render_widget(Paragraph::new(styled_value), row[1]);
}

/// Render appearance settings
fn render_appearance_settings(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;
    let settings = &state.config.settings.ui;

    // Header
    let header = Line::from(vec![Span::styled(
        "Appearance Settings",
        Style::default()
            .fg(theme.accent_primary())
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )]);
    frame.render_widget(
        Paragraph::new(header),
        Rect::new(area.x, area.y, area.width, 1),
    );

    let items = [
        ("Theme", dropdown_value(&settings.theme, theme)),
        (
            "Mouse Support",
            toggle_switch(settings.mouse_enabled, theme),
        ),
        ("Status Bar", toggle_switch(settings.show_status_bar, theme)),
        (
            "Scrollback",
            numeric_value(settings.scrollback_lines, "lines", theme),
        ),
        ("Graph Style", dropdown_value(&settings.graph_style, theme)),
    ];

    let mut y = area.y + 2;
    for (i, (label, value)) in items.iter().enumerate() {
        let row_area = Rect::new(area.x, y, area.width, 1);
        render_setting_row(
            frame,
            row_area,
            label,
            value.clone(),
            i == state.settings_item,
            theme,
        );
        y += 2;
    }
}

/// Render SSH settings
fn render_ssh_settings(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;
    let settings = &state.config.settings.ssh;

    // Header
    let header = Line::from(vec![Span::styled(
        "SSH Settings",
        Style::default()
            .fg(theme.accent_info())
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )]);
    frame.render_widget(
        Paragraph::new(header),
        Rect::new(area.x, area.y, area.width, 1),
    );

    let items = [
        (
            "Timeout",
            numeric_value(settings.connection_timeout, "seconds", theme),
        ),
        (
            "Keep-Alive",
            if settings.keepalive_interval == 0 {
                Line::from(Span::styled(
                    "Disabled",
                    Style::default().fg(theme.fg_dim()),
                ))
            } else {
                numeric_value(settings.keepalive_interval, "seconds", theme)
            },
        ),
        (
            "Reconnect",
            if settings.reconnect_attempts == 0 {
                Line::from(Span::styled(
                    "Disabled",
                    Style::default().fg(theme.fg_dim()),
                ))
            } else {
                numeric_value(settings.reconnect_attempts, "attempts", theme)
            },
        ),
    ];

    let mut y = area.y + 2;
    for (i, (label, value)) in items.iter().enumerate() {
        let row_area = Rect::new(area.x, y, area.width, 1);
        render_setting_row(
            frame,
            row_area,
            label,
            value.clone(),
            i == state.settings_item,
            theme,
        );
        y += 2;
    }
}

/// Render logging settings
fn render_logging_settings(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;
    let settings = &state.config.settings.logging;

    // Header
    let header = Line::from(vec![Span::styled(
        "Logging Settings",
        Style::default()
            .fg(theme.accent_success())
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )]);
    frame.render_widget(
        Paragraph::new(header),
        Rect::new(area.x, area.y, area.width, 1),
    );

    let items = [
        ("Enable Logging", toggle_switch(settings.enabled, theme)),
        ("Log Format", dropdown_value(&settings.format, theme)),
    ];

    let mut y = area.y + 2;
    for (i, (label, value)) in items.iter().enumerate() {
        let row_area = Rect::new(area.x, y, area.width, 1);
        render_setting_row(
            frame,
            row_area,
            label,
            value.clone(),
            i == state.settings_item,
            theme,
        );
        y += 2;
    }
}

/// Render keyboard hints at bottom
fn render_hints(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;

    let hints = if state.settings_dropdown_open {
        vec![("↑/↓", "select"), ("Enter", "confirm"), ("Esc", "cancel")]
    } else {
        vec![
            ("←/→", "category"),
            ("↑/↓", "navigate"),
            ("Enter/Space", "change"),
            ("Esc", "back"),
        ]
    };

    let hint_spans: Vec<Span> = hints
        .iter()
        .flat_map(|(key, desc)| {
            vec![
                Span::styled(
                    format!(" {} ", key),
                    Style::default().fg(theme.bg_main()).bg(theme.accent_info()),
                ),
                Span::styled(format!(" {} ", desc), theme.text_dim()),
            ]
        })
        .collect();

    let hint_line = Line::from(hint_spans);
    let hint_area = Rect::new(
        area.x + 2,
        area.y + area.height - 2,
        area.width.saturating_sub(4),
        1,
    );
    frame.render_widget(Paragraph::new(hint_line), hint_area);
}

/// Render dropdown overlay
fn render_dropdown(frame: &mut Frame, state: &RenderState, content_area: Rect) {
    let theme = &state.theme;

    // Determine dropdown options based on current selection
    let (title, options, current_value) = match state.settings_category {
        0 => match state.settings_item {
            0 => (
                "Theme",
                vec!["tokyo-night", "gruvbox-dark", "dracula", "nord"],
                state.config.settings.ui.theme.as_str(),
            ),
            4 => (
                "Graph Style",
                vec!["braille", "block", "ascii"],
                state.config.settings.ui.graph_style.as_str(),
            ),
            _ => return,
        },
        3 => match state.settings_item {
            1 => (
                "Log Format",
                vec!["timestamped", "raw"],
                state.config.settings.logging.format.as_str(),
            ),
            _ => return,
        },
        _ => return,
    };

    let dropdown_width = 24u16;
    let dropdown_height = (options.len() + 2) as u16;

    // Position dropdown near the selected item
    let x = content_area.x + 24;
    let y = content_area.y + 2 + (state.settings_item as u16 * 2);

    let dropdown_area = Rect::new(
        x.min(content_area.x + content_area.width - dropdown_width),
        y.min(content_area.y + content_area.height - dropdown_height),
        dropdown_width,
        dropdown_height,
    );

    // Clear and draw dropdown box
    frame.render_widget(Clear, dropdown_area);

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(dropdown_area);
    frame.render_widget(block, dropdown_area);

    // Render options
    let mut y = inner.y;
    for option in options {
        let is_current = option == current_value;
        let style = if is_current {
            Style::default()
                .bg(theme.bg_selected())
                .fg(theme.accent_primary())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(theme.bg_panel()).fg(theme.fg_main())
        };

        let prefix = if is_current { "● " } else { "  " };
        let line = Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(option, style),
        ]);

        let option_area = Rect::new(inner.x, y, inner.width, 1);
        frame.render_widget(Clear, option_area);
        frame.render_widget(
            Block::default().style(Style::default().bg(if is_current {
                theme.bg_selected()
            } else {
                theme.bg_panel()
            })),
            option_area,
        );
        frame.render_widget(Paragraph::new(line), option_area);
        y += 1;
    }
}

/// Legacy render function (for App-based rendering)
pub fn render(frame: &mut Frame, app: &crate::app::App, area: Rect) {
    // Convert App to minimal RenderState for rendering
    // This is a compatibility shim
    let theme = &app.theme;

    let title = Line::from(vec![
        Span::styled(" 󰒓 ", theme.title()),
        Span::styled("Settings", theme.title()),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .padding(Padding::uniform(1))
        .style(Style::default().bg(theme.bg_main()));

    frame.render_widget(block, area);
}

/// Render keymap settings (Help info)
fn render_keymap_settings(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;

    // Header
    let header = Line::from(vec![Span::styled(
        "Keymap / Quick Start",
        Style::default()
            .fg(theme.accent_warning())
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )]);
    frame.render_widget(
        Paragraph::new(header),
        Rect::new(area.x, area.y, area.width, 1),
    );

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
            Span::styled("j/k/↑/↓", theme.key_hint()),
            Span::styled(" Navigate lists", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("Tab", theme.key_hint()),
            Span::styled("     Switch focus", theme.text()),
        ]),
        Line::from(vec![
            Span::styled("  ", theme.text_dim()),
            Span::styled("/", theme.key_hint()),
            Span::styled("       Search hosts", theme.text()),
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
    ];

    let help_area = Rect::new(area.x, area.y + 2, area.width, area.height - 2);
    frame.render_widget(Paragraph::new(content), help_area);
}
