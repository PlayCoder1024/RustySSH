//! Settings view

use crate::app::{App, RenderState};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, List, ListItem, Padding};

/// Render settings view with RenderState
pub fn render_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;
    
    let title = Line::from(vec![
        Span::styled(" 󰒓 ", theme.title()),
        Span::styled("Settings", theme.title()),
    ]);
    
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .padding(Padding::uniform(1))
        .style(Style::default().bg(theme.bg_panel()));
    
    let inner = block.inner(area);
    frame.render_widget(block, area);
    
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(70),
        ])
        .split(inner);
    
    // Categories
    let items = vec![
        ListItem::new(Line::from(vec![
            Span::styled(" 󰔎 ", Style::default().fg(theme.accent_primary())),
            Span::styled("Appearance", theme.text_bright()),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(" 󰣀 ", Style::default().fg(theme.accent_info())),
            Span::styled("SSH", theme.text()),
        ])),
    ];
    
    let list = List::new(items).highlight_style(theme.selected());
    frame.render_widget(list, chunks[0]);
    
    // Content
    let content = vec![
        Line::from(vec![
            Span::styled(" Theme: ", theme.text()),
            Span::styled(&state.config.settings.ui.theme, Style::default().fg(theme.accent_primary())),
        ]),
        Line::from(vec![
            Span::styled(" Mouse: ", theme.text()),
            Span::styled(
                if state.config.settings.ui.mouse_enabled { "Enabled" } else { "Disabled" },
                theme.text()
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(content), chunks[1]);
}

/// Render the settings view
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    
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
        .style(Style::default().bg(theme.bg_panel()));
    
    let inner = block.inner(area);
    frame.render_widget(block, area);
    
    // Settings categories
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),  // Category list
            Constraint::Percentage(70),  // Settings content
        ])
        .split(inner);
    
    // Category list
    render_categories(frame, app, chunks[0]);
    
    // Settings content
    render_settings_content(frame, app, chunks[1]);
}

/// Render settings categories
fn render_categories(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    
    let items = vec![
        ListItem::new(Line::from(vec![
            Span::styled(" 󰔎 ", theme.accent_primary()),
            Span::styled("Appearance", theme.text_bright()),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(" 󰣀 ", theme.accent_info()),
            Span::styled("SSH", theme.text()),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(" 󰆍 ", theme.accent_secondary()),
            Span::styled("Terminal", theme.text()),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(" 󰈙 ", theme.accent_success()),
            Span::styled("Logging", theme.text()),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(" 󰌌 ", theme.accent_warning()),
            Span::styled("Keybindings", theme.text()),
        ])),
    ];
    
    let list = List::new(items)
        .highlight_style(theme.selected())
        .highlight_symbol("▶ ");
    
    frame.render_widget(list, area);
}

/// Render settings content
fn render_settings_content(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(theme.border_normal())
        .padding(Padding::horizontal(2));
    
    let inner = block.inner(area);
    frame.render_widget(block, area);
    
    // Appearance settings
    let content = vec![
        Line::from(vec![
            Span::styled(" Appearance", Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED).fg(theme.fg_bright())),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Theme: ", theme.text()),
            Span::styled(&app.config.settings.ui.theme, theme.accent_primary()),
        ]),
        Line::from(vec![
            Span::styled(" Mouse support: ", theme.text()),
            Span::styled(
                if app.config.settings.ui.mouse_enabled { "Enabled" } else { "Disabled" },
                if app.config.settings.ui.mouse_enabled { theme.success() } else { theme.text_dim() }
            ),
        ]),
        Line::from(vec![
            Span::styled(" Show status bar: ", theme.text()),
            Span::styled(
                if app.config.settings.ui.show_status_bar { "Yes" } else { "No" },
                theme.text()
            ),
        ]),
        Line::from(vec![
            Span::styled(" Scrollback lines: ", theme.text()),
            Span::styled(format!("{}", app.config.settings.ui.scrollback_lines), theme.accent_info()),
        ]),
        Line::from(vec![
            Span::styled(" Graph style: ", theme.text()),
            Span::styled(&app.config.settings.ui.graph_style, theme.text()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Available themes:", theme.text_dim()),
        ]),
        Line::from(vec![
            Span::styled("   • tokyo-night (default)", theme.text_dim()),
        ]),
        Line::from(vec![
            Span::styled("   • gruvbox-dark", theme.text_dim()),
        ]),
        Line::from(vec![
            Span::styled("   • dracula", theme.text_dim()),
        ]),
        Line::from(vec![
            Span::styled("   • nord", theme.text_dim()),
        ]),
    ];
    
    let paragraph = Paragraph::new(content);
    frame.render_widget(paragraph, inner);
}
