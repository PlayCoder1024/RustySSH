//! Find overlay widget for searching in terminal sessions

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use crate::tui::Theme;

/// Find overlay for searching within terminal content
pub struct FindOverlay<'a> {
    /// Current search query
    query: &'a str,
    /// Current match index (1-based for display)
    current_match: usize,
    /// Total matches
    total_matches: usize,
    /// Theme
    theme: &'a Theme,
}

impl<'a> FindOverlay<'a> {
    pub fn new(query: &'a str, current_match: usize, total_matches: usize, theme: &'a Theme) -> Self {
        Self {
            query,
            current_match,
            total_matches,
            theme,
        }
    }
}

impl<'a> Widget for FindOverlay<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Calculate overlay position (top-right corner)
        let width = 40u16.min(area.width.saturating_sub(4));
        let height = 3u16;
        let x = area.width.saturating_sub(width + 2);
        let y = 1;
        
        let overlay_area = Rect::new(x, y, width, height);
        
        // Clear the background
        Clear.render(overlay_area, buf);
        
        // Create the overlay block
        let block = Block::default()
            .title(" 󰍉 Find ")
            .borders(Borders::ALL)
            .border_style(self.theme.border_focus())
            .style(Style::default().bg(self.theme.bg_panel()));
        
        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);
        
        // Build the content
        let match_info = if self.total_matches > 0 {
            format!(" ({}/{})", self.current_match + 1, self.total_matches)
        } else if !self.query.is_empty() {
            " (no matches)".to_string()
        } else {
            String::new()
        };
        
        let content = Line::from(vec![
            Span::styled(self.query, self.theme.text()),
            Span::styled("█", Style::default().fg(self.theme.fg_main()).add_modifier(Modifier::SLOW_BLINK)),
            Span::styled(&match_info, self.theme.text_dim()),
        ]);
        
        let paragraph = Paragraph::new(content);
        paragraph.render(inner, buf);
    }
}

/// Render find overlay at specified area
pub fn render_find_overlay(
    frame: &mut Frame,
    area: Rect,
    query: &str,
    current_match: usize,
    total_matches: usize,
    theme: &Theme,
) {
    // Calculate overlay position (top-right corner)
    let width = 40u16.min(area.width.saturating_sub(4));
    let height = 3u16;
    let x = area.x + area.width.saturating_sub(width + 2);
    let y = area.y + 1;
    
    let overlay_area = Rect::new(x, y, width, height);
    
    // Clear the background
    frame.render_widget(Clear, overlay_area);
    
    // Create the overlay block
    let block = Block::default()
        .title(" 󰍉 Find ")
        .borders(Borders::ALL)
        .border_style(theme.border_focus())
        .style(Style::default().bg(theme.bg_panel()));
    
    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);
    
    // Build the content
    let match_info = if total_matches > 0 {
        format!(" ({}/{})", current_match + 1, total_matches)
    } else if !query.is_empty() {
        " (no matches)".to_string()
    } else {
        String::new()
    };
    
    let content = Line::from(vec![
        Span::styled(query, theme.text()),
        Span::styled("█", Style::default().fg(theme.fg_main()).add_modifier(Modifier::SLOW_BLINK)),
        Span::styled(&match_info, theme.text_dim()),
    ]);
    
    let paragraph = Paragraph::new(content);
    frame.render_widget(paragraph, inner);
}
