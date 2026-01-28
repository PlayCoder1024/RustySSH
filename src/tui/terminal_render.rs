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
    let mut lines = Vec::new();
    let (rows, cols) = screen.size();

    for row in 0..rows {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut current_text = String::new();
        let mut current_style: Option<StyledCell> = None;

        for col in 0..cols {
            if let Some(cell) = screen.cell(row, col) {
                let styled_cell = StyledCell::from_vt100_cell(cell);
                
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
                if let Some(prev) = &current_style {
                    if prev.fg.is_none() && prev.bg.is_none() && !prev.bold && !prev.italic && !prev.underline {
                        current_text.push(' ');
                    } else {
                        if !current_text.is_empty() {
                            spans.push(Span::styled(current_text.clone(), prev.to_style()));
                        }
                        current_text = " ".to_string();
                        current_style = Some(StyledCell {
                            content: " ".to_string(),
                            fg: None,
                            bg: None,
                            bold: false,
                            italic: false,
                            underline: false,
                            inverse: false,
                        });
                    }
                } else {
                    current_text.push(' ');
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

        // Trim trailing whitespace spans for cleaner output
        while let Some(last) = spans.last() {
            if last.content.trim().is_empty() {
                spans.pop();
            } else {
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
            }
        }

    lines.push(Line::from(spans));
    }

    lines
}

/// Render a vt100 screen to styled ratatui Lines with selection highlighting
pub fn render_screen_to_lines_with_selection(
    screen: &vt100::Screen,
    selection: Option<((u16, u16), (u16, u16))>,
) -> Vec<Line<'static>> {
    // Fast path: if no selection, use the fast rendering function
    if selection.is_none() {
        return render_screen_to_lines(screen);
    }
    
    // Get selection bounds
    let ((start_row, start_col), (end_row, end_col)) = selection.unwrap();
    
    // First render all lines normally (fast path)
    let mut lines = render_screen_to_lines(screen);
    let (rows, cols) = screen.size();
    
    // Now overlay selection highlighting only on affected rows
    // This is much faster than processing every cell
    for row in start_row..=end_row.min(rows - 1) {
        let row_idx = row as usize;
        if row_idx >= lines.len() {
            continue;
        }
        
        // Determine selection range for this row
        let (sel_start, sel_end) = if start_row == end_row {
            // Single line selection
            (start_col, end_col)
        } else if row == start_row {
            // First line: from start_col to end of line
            (start_col, cols - 1)
        } else if row == end_row {
            // Last line: from start to end_col
            (0, end_col)
        } else {
            // Middle lines: entire row selected
            (0, cols - 1)
        };
        
        // Get the raw content from vt100 for this row
        let mut new_spans: Vec<Span<'static>> = Vec::new();
        let mut current_col = 0u16;
        
        for span in lines[row_idx].spans.iter() {
            let content = span.content.to_string();
            let span_len = content.chars().count() as u16;
            let span_end = current_col + span_len;
            
            // Check if this span overlaps with selection
            if span_end <= sel_start || current_col > sel_end {
                // No overlap, keep span as-is
                new_spans.push(span.clone());
            } else if current_col >= sel_start && span_end <= sel_end + 1 {
                // Entire span is selected
                let style = span.style.add_modifier(ratatui::style::Modifier::REVERSED);
                new_spans.push(Span::styled(content, style));
            } else {
                // Partial overlap - split the span
                let chars: Vec<char> = content.chars().collect();
                let mut i = 0u16;
                
                while i < span_len {
                    let abs_col = current_col + i;
                    let is_selected = abs_col >= sel_start && abs_col <= sel_end;
                    
                    // Find the end of the current segment
                    let mut seg_end = i + 1;
                    while seg_end < span_len {
                        let next_col = current_col + seg_end;
                        let next_selected = next_col >= sel_start && next_col <= sel_end;
                        if next_selected != is_selected {
                            break;
                        }
                        seg_end += 1;
                    }
                    
                    let segment: String = chars[i as usize..seg_end as usize].iter().collect();
                    let style = if is_selected {
                        span.style.add_modifier(ratatui::style::Modifier::REVERSED)
                    } else {
                        span.style
                    };
                    new_spans.push(Span::styled(segment, style));
                    i = seg_end;
                }
            }
            
            current_col = span_end;
        }
        
        lines[row_idx] = Line::from(new_spans);
    }
    
    lines
}
