//! Password input overlay widget

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::RenderState;

/// Render password overlay at specified area
pub fn render_password_overlay(frame: &mut Frame, state: &RenderState, area: Rect) {
    if !state.password_overlay_visible {
        return;
    }

    let theme = &state.theme;

    let has_context = state.password_overlay_context.is_some();
    let has_error = state.password_overlay_error.is_some();

    let mut content_height = 1; // prompt
    if has_context {
        content_height += 1;
    }
    content_height += 3; // input box
    if has_error {
        content_height += 1;
    }
    content_height += 1; // hints

    let width = 70u16.min(area.width.saturating_sub(4));
    let height = (content_height as u16 + 2).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let overlay_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, overlay_area);

    let title = Line::from(vec![
        Span::styled(
            format!(" {} ", state.icons.password),
            Style::default().fg(theme.accent_primary()),
        ),
        Span::styled(&state.password_overlay_title, theme.title()),
        Span::styled(" ", theme.title()),
    ]);

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_primary()))
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    let mut constraints = Vec::new();
    constraints.push(Constraint::Length(1));
    if has_context {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(3));
    if has_error {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let mut idx = 0usize;
    let prompt = Paragraph::new(state.password_overlay_prompt.as_str())
        .style(theme.text_bright())
        .alignment(Alignment::Center);
    frame.render_widget(prompt, chunks[idx]);
    idx += 1;

    if let Some(context) = &state.password_overlay_context {
        let context_line = Paragraph::new(context.as_str())
            .style(theme.text_dim())
            .alignment(Alignment::Center);
        frame.render_widget(context_line, chunks[idx]);
        idx += 1;
    }

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_info()))
        .style(Style::default().bg(theme.bg_panel()));

    let masked_len = state.password_overlay_input.chars().count();
    let masked = "*".repeat(masked_len);
    let input_text = format!("{}_", masked);
    let input = Paragraph::new(input_text)
        .style(Style::default().fg(theme.fg_bright()))
        .alignment(Alignment::Center)
        .block(input_block);
    frame.render_widget(input, chunks[idx]);
    idx += 1;

    if let Some(error) = &state.password_overlay_error {
        let error_line = Paragraph::new(error.as_str())
            .style(theme.error())
            .alignment(Alignment::Center);
        frame.render_widget(error_line, chunks[idx]);
        idx += 1;
    }

    let hints = Paragraph::new(state.password_overlay_hint.as_str())
        .style(theme.text_dim())
        .alignment(Alignment::Center);
    frame.render_widget(hints, chunks[idx]);
}
