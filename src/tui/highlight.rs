//! Terminal content highlighting
//!
//! Provides keyword-based syntax highlighting for terminal output

use aho_corasick::AhoCorasick;
use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::Theme;

/// Terminal highlighting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalHighlightConfig {
    /// Enable terminal highlighting
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Success keywords (displayed in green)
    #[serde(default = "default_success_keywords")]
    pub success_keywords: Vec<String>,

    /// Error keywords (displayed in red)
    #[serde(default = "default_error_keywords")]
    pub error_keywords: Vec<String>,

    /// Warning keywords (displayed in yellow)
    #[serde(default = "default_warning_keywords")]
    pub warning_keywords: Vec<String>,

    /// Info keywords (displayed in blue)
    #[serde(default = "default_info_keywords")]
    pub info_keywords: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_success_keywords() -> Vec<String> {
    vec![
        "ok".to_string(),
        "OK".to_string(),
        "Ok".to_string(),
        "pass".to_string(),
        "PASS".to_string(),
        "passed".to_string(),
        "PASSED".to_string(),
        "success".to_string(),
        "SUCCESS".to_string(),
        "successful".to_string(),
        "done".to_string(),
        "DONE".to_string(),
        "up".to_string(),
        "UP".to_string(),
        "active".to_string(),
        "ACTIVE".to_string(),
        "running".to_string(),
        "RUNNING".to_string(),
        "enabled".to_string(),
        "ENABLED".to_string(),
        "online".to_string(),
        "ONLINE".to_string(),
        "connected".to_string(),
        "CONNECTED".to_string(),
        "true".to_string(),
        "TRUE".to_string(),
        "yes".to_string(),
        "YES".to_string(),
    ]
}

fn default_error_keywords() -> Vec<String> {
    vec![
        "error".to_string(),
        "ERROR".to_string(),
        "Error".to_string(),
        "fail".to_string(),
        "FAIL".to_string(),
        "failed".to_string(),
        "FAILED".to_string(),
        "failure".to_string(),
        "FAILURE".to_string(),
        "down".to_string(),
        "DOWN".to_string(),
        "fatal".to_string(),
        "FATAL".to_string(),
        "critical".to_string(),
        "CRITICAL".to_string(),
        "denied".to_string(),
        "DENIED".to_string(),
        "refused".to_string(),
        "REFUSED".to_string(),
        "timeout".to_string(),
        "TIMEOUT".to_string(),
        "dead".to_string(),
        "DEAD".to_string(),
        "offline".to_string(),
        "OFFLINE".to_string(),
        "inactive".to_string(),
        "INACTIVE".to_string(),
        "disabled".to_string(),
        "DISABLED".to_string(),
        "false".to_string(),
        "FALSE".to_string(),
        "no".to_string(),
        "NO".to_string(),
        "abort".to_string(),
        "ABORT".to_string(),
        "aborted".to_string(),
        "ABORTED".to_string(),
        "panic".to_string(),
        "PANIC".to_string(),
    ]
}

fn default_warning_keywords() -> Vec<String> {
    vec![
        "warn".to_string(),
        "WARN".to_string(),
        "warning".to_string(),
        "WARNING".to_string(),
        "Warning".to_string(),
        "deprecated".to_string(),
        "DEPRECATED".to_string(),
        "caution".to_string(),
        "CAUTION".to_string(),
        "slow".to_string(),
        "SLOW".to_string(),
        "pending".to_string(),
        "PENDING".to_string(),
        "waiting".to_string(),
        "WAITING".to_string(),
        "skip".to_string(),
        "SKIP".to_string(),
        "skipped".to_string(),
        "SKIPPED".to_string(),
    ]
}

fn default_info_keywords() -> Vec<String> {
    vec![
        "info".to_string(),
        "INFO".to_string(),
        "Info".to_string(),
        "note".to_string(),
        "NOTE".to_string(),
        "Note".to_string(),
        "debug".to_string(),
        "DEBUG".to_string(),
        "Debug".to_string(),
        "hint".to_string(),
        "HINT".to_string(),
        "Hint".to_string(),
        "trace".to_string(),
        "TRACE".to_string(),
    ]
}

impl Default for TerminalHighlightConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            success_keywords: default_success_keywords(),
            error_keywords: default_error_keywords(),
            warning_keywords: default_warning_keywords(),
            info_keywords: default_info_keywords(),
        }
    }
}

/// Keyword category for highlighting
#[derive(Debug, Clone, Copy, PartialEq)]
enum KeywordCategory {
    Success,
    Error,
    Warning,
    Info,
}

/// Efficient keyword highlighter using Aho-Corasick algorithm
#[derive(Debug, Clone)]
pub struct Highlighter {
    /// Aho-Corasick automaton for efficient multi-pattern search
    ac: Arc<AhoCorasick>,
    /// Mapping of pattern ID to category
    categories: Vec<KeywordCategory>,
    /// Whether highlighting is enabled
    enabled: bool,
}

impl Highlighter {
    /// Create a new highlighter from configuration
    pub fn new(config: &TerminalHighlightConfig) -> Self {
        let mut patterns = Vec::new();
        let mut categories = Vec::new();

        if config.enabled {
            for kw in &config.success_keywords {
                patterns.push(kw.clone());
                categories.push(KeywordCategory::Success);
            }
            for kw in &config.error_keywords {
                patterns.push(kw.clone());
                categories.push(KeywordCategory::Error);
            }
            for kw in &config.warning_keywords {
                patterns.push(kw.clone());
                categories.push(KeywordCategory::Warning);
            }
            for kw in &config.info_keywords {
                patterns.push(kw.clone());
                categories.push(KeywordCategory::Info);
            }
        }

        // Build automaton with left-most longest match semantics
        let ac = AhoCorasick::builder()
            .match_kind(aho_corasick::MatchKind::LeftmostLongest)
            .build(&patterns)
            .unwrap_or_else(|_| AhoCorasick::new(&patterns).unwrap());

        Self {
            ac: Arc::new(ac),
            categories,
            enabled: config.enabled,
        }
    }

    /// Highlight a single line of terminal output
    pub fn highlight_line<'a>(&self, line: &'a str, theme: &Theme) -> Line<'a> {
        if !self.enabled || line.is_empty() {
            return Line::from(Span::styled(line, theme.text()));
        }

        let mut spans = Vec::new();
        let mut last_end = 0;

        for mat in self.ac.find_iter(line) {
            let start = mat.start();
            let end = mat.end();
            let pattern_id = mat.pattern();

            // Check word boundaries
            // Start boundary: either at start of line, or previous char is not alphanumeric/underscore
            let start_ok = start == 0 || !is_word_char(line.as_bytes()[start - 1]);
            // End boundary: either at end of line, or next char is not alphanumeric/underscore
            let end_ok = end == line.len() || !is_word_char(line.as_bytes()[end]);

            if start_ok && end_ok {
                // Add text before this match
                if start > last_end {
                    spans.push(Span::styled(&line[last_end..start], theme.text()));
                }

                // Add highlighted keyword
                let category = self.categories[pattern_id.as_usize()];
                let style = match category {
                    KeywordCategory::Success => theme.success(),
                    KeywordCategory::Error => theme.error(),
                    KeywordCategory::Warning => theme.warning(),
                    KeywordCategory::Info => theme.info(),
                };
                spans.push(Span::styled(&line[start..end], style));
                last_end = end;
            }
        }

        // Add remaining text
        if last_end < line.len() {
            spans.push(Span::styled(&line[last_end..], theme.text()));
        }

        if spans.is_empty() {
            Line::from(Span::styled(line, theme.text()))
        } else {
            Line::from(spans)
        }
    }

    /// Highlight keywords in an already-styled Line while preserving existing ANSI/VT100 colors
    pub fn highlight_styled_line(&self, line: Line<'static>) -> Line<'static> {
        if !self.enabled {
            return line;
        }

        // Extract full text content from line for keyword matching
        let full_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

        if full_text.is_empty() {
            return line;
        }

        // Find all matches first
        let mut matches = Vec::new();
        for mat in self.ac.find_iter(&full_text) {
            let start = mat.start();
            let end = mat.end();

            // Check word boundaries
            let start_ok = start == 0 || !is_word_char(full_text.as_bytes()[start - 1]);
            let end_ok = end == full_text.len() || !is_word_char(full_text.as_bytes()[end]);

            if start_ok && end_ok {
                matches.push((start, end, self.categories[mat.pattern().as_usize()]));
            }
        }

        if matches.is_empty() {
            return line;
        }

        // Build new spans with keyword highlighting applied on top of existing styles
        let mut new_spans = Vec::new();
        let mut char_offset = 0;

        for span in line.spans {
            let span_text = span.content.to_string();
            let span_len = span_text.len();
            let span_start = char_offset;
            let span_end = char_offset + span_len;

            // Find matches that overlap with this span
            let overlapping: Vec<_> = matches
                .iter()
                .filter(|(start, end, _)| *start < span_end && *end > span_start)
                .cloned()
                .collect();

            if overlapping.is_empty() {
                // No matches in this span, keep original style
                new_spans.push(span);
            } else {
                // Split span based on keyword matches
                let mut pos = 0;
                for (match_start, match_end, category) in overlapping {
                    // Convert to span-local offsets
                    let local_start = match_start.saturating_sub(span_start);
                    let local_end = (match_end - span_start).min(span_len);

                    // Add text before match with original style
                    if local_start > pos {
                        new_spans.push(Span::styled(
                            span_text[pos..local_start].to_string(),
                            span.style,
                        ));
                    }

                    // Add matched text with keyword color, preserving modifiers
                    if local_start < span_len && local_end > local_start {
                        let keyword_color = match category {
                            KeywordCategory::Success => Color::Green,
                            KeywordCategory::Error => Color::Red,
                            KeywordCategory::Warning => Color::Yellow,
                            KeywordCategory::Info => Color::Cyan,
                        };

                        // Apply keyword color but preserve other style attributes (bold, etc)
                        let highlighted_style = span.style.fg(keyword_color);

                        new_spans.push(Span::styled(
                            span_text[local_start.max(pos)..local_end].to_string(),
                            highlighted_style,
                        ));
                    }

                    pos = local_end;
                }

                // Add remaining text with original style
                if pos < span_len {
                    new_spans.push(Span::styled(span_text[pos..].to_string(), span.style));
                }
            }

            char_offset = span_end;
        }

        Line::from(new_spans)
    }
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn test_highlight_line() {
        let config = TerminalHighlightConfig {
            enabled: true,
            success_keywords: vec!["ok".to_string()],
            error_keywords: vec![],
            warning_keywords: vec![],
            info_keywords: vec![],
        };
        let highlighter = Highlighter::new(&config);
        let theme = Theme::default();

        let line = highlighter.highlight_line("status: ok", &theme);
        assert!(line.spans.len() >= 2);
        let last_span = line.spans.last().unwrap();
        assert_eq!(last_span.content, "ok");
        // Check success color (green by default in theme usually, or at least different from text)
        // theme.success() returns a Style. Let's just check it matches.
        assert_eq!(last_span.style, theme.success());
    }

    #[test]
    fn test_highlight_styled_line() {
        let config = TerminalHighlightConfig {
            enabled: true,
            success_keywords: vec!["success".to_string()],
            error_keywords: vec![],
            warning_keywords: vec![],
            info_keywords: vec![],
        };
        let highlighter = Highlighter::new(&config);

        let input = Line::from(vec![Span::styled(
            "test success",
            Style::default().fg(Color::White),
        )]);
        let output = highlighter.highlight_styled_line(input);

        let spans = output.spans;
        // Start span
        assert_eq!(spans[0].content, "test ");
        assert_eq!(spans[0].style.fg, Some(Color::White));

        // Highlighted span
        assert_eq!(spans[1].content, "success");
        assert_eq!(spans[1].style.fg, Some(Color::Green));
    }
}
