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

/// Styled cell information from vt100
#[derive(Debug, Clone, PartialEq)]
pub struct StyledCell {
    pub content: String,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl StyledCell {
    /// Create from vt100 cell
    pub fn from_vt100_cell(cell: &vt100::Cell) -> Self {
        Self {
            content: cell.contents().to_string(),
            fg: convert_color(cell.fgcolor()),
            bg: convert_color(cell.bgcolor()),
            bold: cell.bold(),
            italic: cell.italic(),
            underline: cell.underline(),
            inverse: cell.inverse(),
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

    /// Check if two cells have the same style (for merging)
    pub fn same_style(&self, other: &Self) -> bool {
        self.fg == other.fg
            && self.bg == other.bg
            && self.bold == other.bold
            && self.italic == other.italic
            && self.underline == other.underline
            && self.inverse == other.inverse
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
    let selection_unwrapped = selection.map(|((sr, sc), (er, ec))| (sr, sc, er, ec));

    for row in 0..rows {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut current_text = String::new();
        let mut current_style: Option<StyledCell> = None;

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

            if let Some(cell) = screen.cell(row, col) {
                let mut styled_cell = StyledCell::from_vt100_cell(cell);
                
                // Apply selection highlighting by inverting colors
                if is_selected {
                    styled_cell.inverse = !styled_cell.inverse;
                }
                
                // Get content (handle empty cells)
                let content = if styled_cell.content.is_empty() {
                    " ".to_string()
                } else {
                    styled_cell.content.clone()
                };

                match &current_style {
                    Some(prev) if prev.same_style(&styled_cell) => {
                        // Same style, append to current text
                        current_text.push_str(&content);
                    }
                    _ => {
                        // Different style, flush previous and start new
                        if !current_text.is_empty() {
                            if let Some(prev) = &current_style {
                                spans.push(Span::styled(current_text.clone(), prev.to_style()));
                            }
                        }
                        current_text = content;
                        current_style = Some(styled_cell);
                    }
                }
            } else {
                // No cell, add space
                // Create a default style for empty space, potentially selected
                let mut empty_style = StyledCell {
                    content: " ".to_string(),
                    fg: None,
                    bg: None,
                    bold: false,
                    italic: false,
                    underline: false,
                    inverse: false,
                };

                if is_selected {
                    empty_style.inverse = true;
                }

                if let Some(prev) = &current_style {
                    if prev.same_style(&empty_style) {
                        current_text.push(' ');
                    } else {
                        if !current_text.is_empty() {
                            spans.push(Span::styled(current_text.clone(), prev.to_style()));
                        }
                        current_text = " ".to_string();
                        current_style = Some(empty_style);
                    }
                } else {
                    current_text.push(' ');
                    current_style = Some(empty_style);
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
