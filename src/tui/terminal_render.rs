//! Terminal rendering with true color support
//!
//! Converts vt100 screen content to ratatui styled lines,
//! preserving ANSI colors, bold, italic, underline, etc.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Convert vt100 Color to ratatui Color
pub fn convert_color(color: vt100::Color) -> Option<Color> {
    match color {
        vt100::Color::Default => None,
        vt100::Color::Idx(idx) => Some(Color::Indexed(idx)),
        vt100::Color::Rgb(r, g, b) => Some(Color::Rgb(r, g, b)),
    }
}

/// Styled cell information from vt100 (style only)
#[derive(Debug, Clone, PartialEq)]
pub struct CellStyle {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl CellStyle {
    /// Create from vt100 cell
    pub fn from_vt100_cell(cell: &vt100::Cell) -> Self {
        Self {
            fg: convert_color(cell.fgcolor()),
            bg: convert_color(cell.bgcolor()),
            bold: cell.bold(),
            italic: cell.italic(),
            underline: cell.underline(),
            inverse: cell.inverse(),
        }
    }

    /// Default style for empty cells
    pub fn default_style() -> Self {
        Self {
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            underline: false,
            inverse: false,
        }
    }

    /// Convert to ratatui Style
    pub fn to_style(&self) -> Style {
        let mut style = Style::default();

        if self.inverse {
            // Swap fg/bg for inverse
            if let Some(fg) = self.fg {
                style = style.bg(fg);
            }
            if let Some(bg) = self.bg {
                style = style.fg(bg);
            } else {
                // If no bg specified, use white fg for inverse
                style = style.fg(Color::White);
            }
        } else {
            if let Some(fg) = self.fg {
                style = style.fg(fg);
            }
            if let Some(bg) = self.bg {
                style = style.bg(bg);
            }
        }

        let mut modifiers = Modifier::empty();
        if self.bold {
            modifiers |= Modifier::BOLD;
        }
        if self.italic {
            modifiers |= Modifier::ITALIC;
        }
        if self.underline {
            modifiers |= Modifier::UNDERLINED;
        }

        if !modifiers.is_empty() {
            style = style.add_modifier(modifiers);
        }

        style
    }
}

/// Render a vt100 screen to styled ratatui Lines
pub fn render_screen_to_lines(screen: &vt100::Screen) -> Vec<Line<'static>> {
    render_screen_to_lines_impl(screen, None)
}

/// Render a vt100 screen to styled ratatui Lines with selection highlighting
pub fn render_screen_to_lines_with_selection(
    screen: &vt100::Screen,
    selection: Option<((u16, u16), (u16, u16))>,
) -> Vec<Line<'static>> {
    render_screen_to_lines_impl(screen, selection)
}

fn render_screen_to_lines_impl(
    screen: &vt100::Screen,
    selection: Option<((u16, u16), (u16, u16))>,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let (rows, cols) = screen.size();

    // Unwrap selection for faster access in loop
    // (start_row, start_col, end_row, end_col)
    let selection_unwrapped = selection.map(|((sr, sc), (er, ec))| (sr, sc, er, ec));

    for row in 0..rows {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut current_text = String::with_capacity(cols as usize);
        let mut current_style: Option<CellStyle> = None;

        for col in 0..cols {
            // Check selection state
            let is_selected = if let Some((sr, sc, er, ec)) = selection_unwrapped {
                if row >= sr && row <= er {
                    if sr == er {
                        // Single line
                        col >= sc && col <= ec
                    } else if row == sr {
                        // First line
                        col >= sc
                    } else if row == er {
                        // Last line
                        col <= ec
                    } else {
                        // Middle line
                        true
                    }
                } else {
                    false
                }
            } else {
                false
            };

            let cell = screen.cell(row, col);

            // Skip wide continuation cells (second cell of a CJK/wide character).
            // These are empty placeholder cells; ratatui handles the width natively.
            if let Some(ref c) = cell {
                if c.is_wide_continuation() {
                    continue;
                }
            }

            let (content_str, mut style) = if let Some(cell) = cell {
                let s = CellStyle::from_vt100_cell(&cell);
                let c = cell.contents();
                let content = if c.is_empty() { " " } else { c };
                (content, s)
            } else {
                (" ", CellStyle::default_style())
            };

            // Apply selection highlighting by inverting colors
            if is_selected {
                style.inverse = !style.inverse;
            }

            match &current_style {
                Some(prev) if *prev == style => {
                    // Same style, append to current text
                    current_text.push_str(content_str);
                }
                _ => {
                    // Different style, flush previous and start new
                    if !current_text.is_empty() {
                        if let Some(prev) = &current_style {
                            spans.push(Span::styled(current_text.clone(), prev.to_style()));
                        }
                    }
                    current_text.clear();
                    current_text.push_str(content_str);
                    current_style = Some(style);
                }
            }
        }

        // Flush remaining text
        if !current_text.is_empty() {
            if let Some(prev) = &current_style {
                spans.push(Span::styled(current_text, prev.to_style()));
            } else {
                spans.push(Span::raw(current_text));
            }
        }

        // Trim trailing whitespace spans for cleaner output, BUT NOT if they are selected
        // We only trim if the style is default (not selected)
        while let Some(last) = spans.last() {
            // Check if style is "default" (no inverse, no colors)
            // If selected (inverse=true), we must preserve it
            let is_default_style = last.style == Style::default();

            if is_default_style && last.content.trim().is_empty() {
                spans.pop();
            } else if is_default_style {
                // Trim the last span's trailing whitespace
                if let Some(last_span) = spans.last_mut() {
                    let trimmed = last_span.content.trim_end().to_string();
                    if trimmed.is_empty() {
                        spans.pop();
                    } else {
                        *last_span = Span::styled(trimmed, last_span.style);
                    }
                }
                break;
            } else {
                break;
            }
        }

        lines.push(Line::from(spans));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Style};

    #[test]
    fn test_render_simple_text() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"Hello world");
        let screen = parser.screen();
        let lines = render_screen_to_lines(screen);

        let first_line = &lines[0];
        assert_eq!(first_line.spans.len(), 1);
        assert_eq!(first_line.spans[0].content, "Hello world");
    }

    #[test]
    fn test_render_colored_text() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // \x1b[31mRed\x1b[0m \x1b[32mGreen\x1b[0m
        parser.process(b"\x1b[31mRed\x1b[0m \x1b[32mGreen\x1b[0m");
        let screen = parser.screen();
        let lines = render_screen_to_lines(screen);

        let first_line = &lines[0];
        // "Red" (red), " " (default), "Green" (green)
        assert_eq!(first_line.spans.len(), 3);
        assert_eq!(first_line.spans[0].content, "Red");
        // vt100 returns Indexed(1) for Red
        assert_eq!(first_line.spans[0].style.fg, Some(Color::Indexed(1)));

        assert_eq!(first_line.spans[1].content, " ");
        assert_eq!(first_line.spans[1].style, Style::default());

        assert_eq!(first_line.spans[2].content, "Green");
        // vt100 returns Indexed(2) for Green
        assert_eq!(first_line.spans[2].style.fg, Some(Color::Indexed(2)));
    }

    #[test]
    fn test_cell_style() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"\x1b[1;31mBold Red"); // Bold + Red
        let screen = parser.screen();
        let cell = screen.cell(0, 0).unwrap();
        let style = CellStyle::from_vt100_cell(cell);

        assert_eq!(style.fg, Some(Color::Indexed(1)));
        assert!(style.bold);

        let ratatui_style = style.to_style();
        assert_eq!(ratatui_style.fg, Some(Color::Indexed(1)));
        // Note: ratatui's Modifier is a bitflag.
        assert!(ratatui_style
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD));
    }
}
