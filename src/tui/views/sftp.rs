//! SFTP file browser view

use crate::app::{App, FilePaneSnapshot, RenderState};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Table};

fn visible_range(total_rows: usize, cursor: usize, viewport_rows: usize) -> (usize, usize) {
    if total_rows == 0 || viewport_rows == 0 {
        return (0, 0);
    }

    let clamped_cursor = cursor.min(total_rows - 1);
    let half_view = viewport_rows / 2;
    let mut start = clamped_cursor.saturating_sub(half_view);
    let max_start = total_rows.saturating_sub(viewport_rows);
    start = start.min(max_start);
    let end = (start + viewport_rows).min(total_rows);
    (start, end)
}

/// Render the SFTP view with RenderState
pub fn render_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // File panes
            Constraint::Length(4), // Transfer queue
        ])
        .split(area);

    let pane_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    // Get file browser data
    if let Some(browser) = &state.file_browser {
        // Left pane (local)
        render_pane_with_data(
            frame,
            theme,
            pane_chunks[0],
            &browser.left,
            browser.active_is_left,
        );
        // Right pane (remote)
        render_pane_with_data(
            frame,
            theme,
            pane_chunks[1],
            &browser.right,
            !browser.active_is_left,
        );
    } else {
        // No file browser - show placeholder
        render_placeholder_pane(frame, theme, pane_chunks[0], "Local", true);
        render_placeholder_pane(frame, theme, pane_chunks[1], "Remote", false);
    }

    // Transfer queue with progress
    render_transfer_queue_state(frame, state, chunks[1]);
}

/// Render a file pane with actual data
fn render_pane_with_data(
    frame: &mut Frame,
    theme: &crate::tui::Theme,
    area: Rect,
    pane: &FilePaneSnapshot,
    is_active: bool,
) {
    let label = if pane.is_remote { "Remote" } else { "Local" };

    // Truncate path if too long
    let path_display = if pane.path.len() > 30 {
        format!("...{}", &pane.path[pane.path.len() - 27..])
    } else {
        pane.path.clone()
    };

    let title = Line::from(vec![
        Span::styled(" 󰉋 ", theme.title()),
        Span::styled(format!("{} [{}]", label, path_display), theme.title()),
        Span::styled(" ", theme.title()),
    ]);

    let border_style = if is_active {
        theme.border_focus()
    } else {
        theme.border_normal()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let total_rows = pane.entries.len();
    let cursor = if total_rows == 0 {
        0
    } else {
        pane.cursor.min(total_rows - 1)
    };
    let visible_rows = inner.height as usize;
    let (start, end) = visible_range(total_rows, cursor, visible_rows);

    // Build table rows from the visible window only.
    let rows: Vec<Row> = pane.entries[start..end]
        .iter()
        .enumerate()
        .map(|(offset, entry)| {
            let idx = start + offset;
            let icon = if entry.is_dir { "📁 " } else { "📄 " };

            let style = if idx == cursor && is_active {
                theme.selected()
            } else if entry.selected {
                Style::default()
                    .fg(theme.accent_primary())
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.text()
            };

            Row::new(vec![
                Cell::from(format!("{}{}", icon, entry.name)).style(style),
                Cell::from(entry.size_display.clone()).style(theme.text_dim()),
            ])
        })
        .collect();

    let widths = [Constraint::Min(20), Constraint::Length(10)];
    let table = Table::new(rows, widths);
    frame.render_widget(table, inner);
}

/// Render placeholder pane when no SFTP connection
fn render_placeholder_pane(
    frame: &mut Frame,
    theme: &crate::tui::Theme,
    area: Rect,
    label: &str,
    is_active: bool,
) {
    let title = Line::from(vec![
        Span::styled(" 󰉋 ", theme.title()),
        Span::styled(label, theme.title()),
    ]);

    let border_style = if is_active {
        theme.border_focus()
    } else {
        theme.border_normal()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let placeholder = Paragraph::new(Line::from(vec![
        Span::styled("Connect to a host first, then press ", theme.text_dim()),
        Span::styled("f", theme.key_hint()),
        Span::styled(" for SFTP", theme.text_dim()),
    ]))
    .alignment(Alignment::Center);

    // Center vertically
    let centered = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Length(1),
            Constraint::Percentage(45),
        ])
        .split(inner);

    frame.render_widget(placeholder, centered[1]);
}

/// Render transfer queue with progress
fn render_transfer_queue_state(frame: &mut Frame, state: &RenderState, area: Rect) {
    let theme = &state.theme;
    let transfer_info = &state.transfer_info;

    let pending_count = transfer_info.pending_count + transfer_info.active_count;

    let title = Line::from(vec![
        Span::styled(" 󰇚 ", theme.title()),
        Span::styled("Transfers", theme.title()),
        if pending_count > 0 {
            Span::styled(format!(" ({})", pending_count), theme.accent_primary())
        } else {
            Span::raw("")
        },
        Span::styled(" ", theme.title()),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_normal())
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if transfer_info.active_transfers.is_empty() {
        // No active transfers - show help
        let content = Line::from(vec![
            Span::styled("No active transfers", theme.text_dim()),
            Span::raw("  │  "),
            Span::styled("c", theme.key_hint()),
            Span::styled("/F5 copy  ", theme.text_dim()),
            Span::styled("m", theme.key_hint()),
            Span::styled("/F6 move  ", theme.text_dim()),
            Span::styled("Tab", theme.key_hint()),
            Span::styled(" switch pane  ", theme.text_dim()),
            Span::styled("Enter", theme.key_hint()),
            Span::styled(" open", theme.text_dim()),
        ]);
        frame.render_widget(Paragraph::new(content), inner);
    } else {
        // Show active transfers
        let transfer = &transfer_info.active_transfers[0];
        let direction_icon = if transfer.is_upload {
            "⬆️"
        } else {
            "⬇️"
        };

        // Progress bar area
        let progress_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(30),
                Constraint::Length(15),
                Constraint::Length(12),
            ])
            .split(inner);

        // Filename with icon
        let filename_line = Line::from(vec![
            Span::raw(direction_icon),
            Span::raw(" "),
            Span::styled(&transfer.filename, theme.text()),
            Span::styled(
                format!(" {:.0}%", transfer.progress),
                theme.accent_primary(),
            ),
        ]);
        frame.render_widget(Paragraph::new(filename_line), progress_area[0]);

        // Speed
        let speed_line = Line::from(vec![Span::styled(
            &transfer.speed_display,
            theme.text_dim(),
        )]);
        frame.render_widget(
            Paragraph::new(speed_line).alignment(Alignment::Right),
            progress_area[1],
        );

        // ETA
        let eta_line = Line::from(vec![Span::styled(&transfer.eta_display, theme.text_dim())]);
        frame.render_widget(
            Paragraph::new(eta_line).alignment(Alignment::Right),
            progress_area[2],
        );
    }
}

/// Render the SFTP view (legacy function using App directly)
pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let _theme = &app.theme;

    // Dual pane layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // File panes
            Constraint::Length(3), // Transfer queue
        ])
        .split(area);

    let pane_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    // Left pane (local)
    render_file_pane(frame, app, pane_chunks[0], true, true);

    // Right pane (remote)
    render_file_pane(frame, app, pane_chunks[1], false, false);

    // Transfer queue
    render_transfer_queue(frame, app, chunks[1]);
}

/// Render a file pane (legacy)
fn render_file_pane(frame: &mut Frame, app: &App, area: Rect, is_left: bool, is_active: bool) {
    let theme = &app.theme;

    let label = if is_left { "Local" } else { "Remote" };
    let path = if is_left { "/home/user" } else { "/" }; // Placeholder

    let title = Line::from(vec![
        Span::styled(" 󰉋 ", theme.title()),
        Span::styled(format!("{} ({})", label, path), theme.title()),
        Span::styled(" ", theme.title()),
    ]);

    let border_style = if is_active {
        theme.border_focus()
    } else {
        theme.border_normal()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Sample file listing
    let rows = vec![
        create_file_row("..", "<DIR>", "", theme, false),
        create_file_row("Documents", "<DIR>", "2024-01-10", theme, false),
        create_file_row("Downloads", "<DIR>", "2024-01-09", theme, false),
        create_file_row("config.yaml", "2.1K", "2024-01-08", theme, true),
        create_file_row("notes.md", "512B", "2024-01-07", theme, false),
    ];

    let widths = [
        Constraint::Min(20),
        Constraint::Length(8),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .header(Row::new(vec![
            Cell::from("Name").style(theme.text_dim()),
            Cell::from("Size").style(theme.text_dim()),
            Cell::from("Modified").style(theme.text_dim()),
        ]))
        .highlight_style(theme.selected());

    frame.render_widget(table, inner);
}

/// Create a file row (legacy)
fn create_file_row<'a>(
    name: &'a str,
    size: &'a str,
    date: &'a str,
    theme: &crate::tui::Theme,
    selected: bool,
) -> Row<'a> {
    let icon = if size == "<DIR>" { "📁 " } else { "📄 " };

    let style = if selected {
        theme.selected()
    } else {
        theme.text()
    };

    Row::new(vec![
        Cell::from(format!("{}{}", icon, name)).style(style),
        Cell::from(size).style(theme.text_dim()),
        Cell::from(date).style(theme.text_dim()),
    ])
}

/// Render the transfer queue (legacy)
fn render_transfer_queue(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;

    let title = Line::from(vec![
        Span::styled(" 󰇚 ", theme.title()),
        Span::styled("Transfers", theme.title()),
        Span::styled(" ", theme.title()),
    ]);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_normal())
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(theme.bg_panel()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Sample transfer status
    let content = Line::from(vec![
        Span::styled("No active transfers", theme.text_dim()),
        Span::raw("  │  "),
        Span::styled("Press ", theme.text_dim()),
        Span::styled("c", theme.key_hint()),
        Span::styled(" to copy,  ", theme.text_dim()),
        Span::styled("m", theme.key_hint()),
        Span::styled(" to move", theme.text_dim()),
    ]);

    let paragraph = Paragraph::new(content);
    frame.render_widget(paragraph, inner);
}

#[cfg(test)]
mod tests {
    use super::visible_range;

    #[test]
    fn visible_range_stays_at_top_when_cursor_near_start() {
        assert_eq!(visible_range(50, 0, 10), (0, 10));
        assert_eq!(visible_range(50, 3, 10), (0, 10));
    }

    #[test]
    fn visible_range_follows_cursor_in_middle() {
        assert_eq!(visible_range(100, 40, 10), (35, 45));
    }

    #[test]
    fn visible_range_clamps_at_bottom() {
        assert_eq!(visible_range(50, 49, 10), (40, 50));
    }

    #[test]
    fn visible_range_handles_empty_or_tiny_inputs() {
        assert_eq!(visible_range(0, 0, 10), (0, 0));
        assert_eq!(visible_range(5, 2, 10), (0, 5));
        assert_eq!(visible_range(5, 99, 0), (0, 0));
    }
}
