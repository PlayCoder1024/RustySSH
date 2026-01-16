//! Terminal content highlighting
//!
//! Provides keyword-based syntax highlighting for terminal output

use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use serde::{Deserialize, Serialize};

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
    None,
}

/// Highlight a single line of terminal output
/// 
/// Scans for keywords and returns a styled Line with colored spans
pub fn highlight_line<'a>(line: &'a str, theme: &Theme, config: &TerminalHighlightConfig) -> Line<'a> {
    if !config.enabled || line.is_empty() {
        return Line::from(Span::styled(line, theme.text()));
    }

    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut remaining = line;
    let mut last_end = 0;

    // Find all keyword matches
    let mut matches: Vec<(usize, usize, KeywordCategory)> = Vec::new();

    // Check each keyword category
    for keyword in &config.success_keywords {
        find_keyword_matches(line, keyword, KeywordCategory::Success, &mut matches);
    }
    for keyword in &config.error_keywords {
        find_keyword_matches(line, keyword, KeywordCategory::Error, &mut matches);
    }
    for keyword in &config.warning_keywords {
        find_keyword_matches(line, keyword, KeywordCategory::Warning, &mut matches);
    }
    for keyword in &config.info_keywords {
        find_keyword_matches(line, keyword, KeywordCategory::Info, &mut matches);
    }

    // Sort by position
    matches.sort_by_key(|(start, _, _)| *start);

    // Remove overlapping matches (keep first)
    let mut filtered_matches: Vec<(usize, usize, KeywordCategory)> = Vec::new();
    for m in matches {
        if filtered_matches.is_empty() || m.0 >= filtered_matches.last().unwrap().1 {
            filtered_matches.push(m);
        }
    }

    // Build spans
    for (start, end, category) in filtered_matches {
        // Add text before this match
        if start > last_end {
            spans.push(Span::styled(&line[last_end..start], theme.text()));
        }

        // Add highlighted keyword
        let style = match category {
            KeywordCategory::Success => theme.success(),
            KeywordCategory::Error => theme.error(),
            KeywordCategory::Warning => theme.warning(),
            KeywordCategory::Info => theme.info(),
            KeywordCategory::None => theme.text(),
        };
        spans.push(Span::styled(&line[start..end], style));
        last_end = end;
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

/// Find all occurrences of a keyword in text with word boundary checking
fn find_keyword_matches(
    text: &str,
    keyword: &str,
    category: KeywordCategory,
    matches: &mut Vec<(usize, usize, KeywordCategory)>,
) {
    let keyword_len = keyword.len();
    let text_len = text.len();
    
    let mut search_start = 0;
    while search_start < text_len {
        if let Some(pos) = text[search_start..].find(keyword) {
            let abs_pos = search_start + pos;
            let end_pos = abs_pos + keyword_len;
            
            // Check word boundaries
            let start_ok = abs_pos == 0 || !text.chars().nth(abs_pos - 1).map_or(false, |c| c.is_alphanumeric() || c == '_');
            let end_ok = end_pos >= text_len || !text.chars().nth(end_pos).map_or(false, |c| c.is_alphanumeric() || c == '_');
            
            if start_ok && end_ok {
                matches.push((abs_pos, end_pos, category));
            }
            
            search_start = end_pos;
        } else {
            break;
        }
    }
}
