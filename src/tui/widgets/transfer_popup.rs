use crate::app::TransferQueueSnapshot;
use crate::tui::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Gauge, ListItem, List, Padding};

/// Widget to display active file transfers
pub struct TransferPopup<'a> {
    transfer_info: &'a TransferQueueSnapshot,
    theme: &'a Theme,
    area: Rect,
}

impl<'a> TransferPopup<'a> {
    /// Create a new transfer popup
    pub fn new(transfer_info: &'a TransferQueueSnapshot, theme: &'a Theme, area: Rect) -> Self {
        Self {
            transfer_info,
            theme,
            area,
        }
    }

    /// Render the popup
    pub fn render(self, frame: &mut Frame) {
        // Only render if there are active transfers or we want to show pending
        if self.transfer_info.active_transfers.is_empty() && self.transfer_info.pending_count == 0 {
            return;
        }

        // Calculate popup area (bottom right corner, fixed width)
        let width = 60;
        let height = (self.transfer_info.active_transfers.len() * 3 + 2).min(15) as u16; // 3 lines per transfer + padding
        
        // Position at bottom right, above status bar
        let area = Rect::new(
            self.area.width.saturating_sub(width + 2),
            self.area.height.saturating_sub(height + 2), 
            width,
            height,
        );

        // Clear background
        frame.render_widget(Clear, area);

        // Main block
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " Transfers ({} Active, {} Pending) ",
                self.transfer_info.active_count, self.transfer_info.pending_count
            ))
            .style(self.theme.popup_border());

        frame.render_widget(block.clone(), area);

        // Inner area
        let inner = block.inner(area);

        // Layout for transfers
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                self.transfer_info
                    .active_transfers
                    .iter()
                    .map(|_| Constraint::Length(3))
                    .collect::<Vec<_>>(),
            )
            .split(inner);

        // Render each active transfer
        for (i, transfer) in self.transfer_info.active_transfers.iter().enumerate() {
            if i >= layout.len() {
                break;
            }
            
            let chunk = layout[i];
            
            // Top line: Filename + Action
            let action = if transfer.is_upload { "↑ Uploading" } else { "↓ Downloading" };
            let title = format!("{} {}", action, transfer.filename);
            
            let label_area = Rect::new(chunk.x, chunk.y, chunk.width, 1);
            frame.render_widget(
                ratatui::widgets::Paragraph::new(title)
                    .style(self.theme.text()),
                label_area
            );

            // Middle line: Progress Bar
            let gauge_area = Rect::new(chunk.x, chunk.y + 1, chunk.width, 1);
            let gauge = Gauge::default()
                .gauge_style(self.theme.progress_bar())
                .ratio(transfer.progress / 100.0)
                .label(format!("{:.1}%", transfer.progress));
            
            frame.render_widget(gauge, gauge_area);

            // Bottom line: Speed + ETA
            let details = format!("{} • ETA: {}", transfer.speed_display, transfer.eta_display);
            let details_area = Rect::new(chunk.x, chunk.y + 2, chunk.width, 1);
            frame.render_widget(
                ratatui::widgets::Paragraph::new(details)
                    .style(self.theme.text_dim())
                    .alignment(Alignment::Right),
                details_area
            );
        }
    }
}
