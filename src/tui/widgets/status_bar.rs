//! Status bar widget

use ratatui::prelude::*;
use ratatui::widgets::Widget;

/// Status bar widget
pub struct StatusBar<'a> {
    left_text: &'a str,
    right_text: &'a str,
    style: Style,
}

impl<'a> StatusBar<'a> {
    /// Create a new status bar
    pub fn new() -> Self {
        Self {
            left_text: "",
            right_text: "",
            style: Style::default(),
        }
    }

    /// Set left text
    pub fn left(mut self, text: &'a str) -> Self {
        self.left_text = text;
        self
    }

    /// Set right text
    pub fn right(mut self, text: &'a str) -> Self {
        self.right_text = text;
        self
    }

    /// Set style
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl<'a> Default for StatusBar<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fill background
        for x in area.x..area.x + area.width {
            buf.get_mut(x, area.y).set_style(self.style);
        }

        // Render left text
        let left_span = Span::styled(self.left_text, self.style);
        buf.set_span(area.x + 1, area.y, &left_span, area.width.saturating_sub(1));

        // Render right text (right-aligned)
        let right_len = self.right_text.len() as u16;
        if right_len < area.width {
            let right_x = area.x + area.width - right_len - 1;
            let right_span = Span::styled(self.right_text, self.style);
            buf.set_span(right_x, area.y, &right_span, right_len);
        }
    }
}
