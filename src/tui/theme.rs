//! btop-inspired color theme

use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};

/// Theme configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Theme name
    pub name: String,
    /// Colors
    pub colors: ThemeColors,
}

/// Theme color palette
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeColors {
    // Background colors
    pub bg_main: String,
    pub bg_panel: String,
    pub bg_highlight: String,
    pub bg_selected: String,

    // Foreground colors
    pub fg_main: String,
    pub fg_dim: String,
    pub fg_bright: String,

    // Accent colors
    pub accent_primary: String,
    pub accent_secondary: String,
    pub accent_success: String,
    pub accent_warning: String,
    pub accent_error: String,
    pub accent_info: String,

    // Graph colors
    pub graph_1: String,
    pub graph_2: String,
    pub graph_3: String,
    pub graph_4: String,

    // Border colors
    pub border_focused: String,
    pub border_unfocused: String,
}

impl Default for Theme {
    /// Tokyo Night inspired theme (similar to btop's aesthetic)
    fn default() -> Self {
        Self {
            name: "tokyo-night".to_string(),
            colors: ThemeColors {
                // Background - dark navy
                bg_main: "#1a1b26".to_string(),
                bg_panel: "#24283b".to_string(),
                bg_highlight: "#292e42".to_string(),
                bg_selected: "#364a82".to_string(),

                // Foreground
                fg_main: "#a9b1d6".to_string(),
                fg_dim: "#565f89".to_string(),
                fg_bright: "#c0caf5".to_string(),

                // Accent colors
                accent_primary: "#7aa2f7".to_string(),   // Blue
                accent_secondary: "#bb9af7".to_string(), // Purple
                accent_success: "#9ece6a".to_string(),   // Green
                accent_warning: "#e0af68".to_string(),   // Orange
                accent_error: "#f7768e".to_string(),     // Red/Pink
                accent_info: "#7dcfff".to_string(),      // Cyan

                // Graph colors (for visualization)
                graph_1: "#7aa2f7".to_string(), // Blue
                graph_2: "#bb9af7".to_string(), // Purple
                graph_3: "#2ac3de".to_string(), // Teal
                graph_4: "#9ece6a".to_string(), // Green

                // Borders
                border_focused: "#7aa2f7".to_string(),
                border_unfocused: "#3b4261".to_string(),
            },
        }
    }
}

impl Theme {
    /// Parse hex color to ratatui Color
    pub fn parse_color(hex: &str) -> Color {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return Color::Reset;
        }

        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);

        Color::Rgb(r, g, b)
    }

    // --- Background styles ---

    pub fn bg_main(&self) -> Color {
        Self::parse_color(&self.colors.bg_main)
    }

    pub fn bg_panel(&self) -> Color {
        Self::parse_color(&self.colors.bg_panel)
    }

    pub fn bg_highlight(&self) -> Color {
        Self::parse_color(&self.colors.bg_highlight)
    }

    pub fn bg_selected(&self) -> Color {
        Self::parse_color(&self.colors.bg_selected)
    }

    // --- Foreground colors ---

    pub fn fg_main(&self) -> Color {
        Self::parse_color(&self.colors.fg_main)
    }

    pub fn fg_dim(&self) -> Color {
        Self::parse_color(&self.colors.fg_dim)
    }

    pub fn fg_bright(&self) -> Color {
        Self::parse_color(&self.colors.fg_bright)
    }

    // --- Accent colors ---

    pub fn accent_primary(&self) -> Color {
        Self::parse_color(&self.colors.accent_primary)
    }

    pub fn accent_secondary(&self) -> Color {
        Self::parse_color(&self.colors.accent_secondary)
    }

    pub fn accent_success(&self) -> Color {
        Self::parse_color(&self.colors.accent_success)
    }

    pub fn accent_warning(&self) -> Color {
        Self::parse_color(&self.colors.accent_warning)
    }

    pub fn accent_error(&self) -> Color {
        Self::parse_color(&self.colors.accent_error)
    }

    pub fn accent_info(&self) -> Color {
        Self::parse_color(&self.colors.accent_info)
    }

    // --- Border colors ---

    pub fn border_focused(&self) -> Color {
        Self::parse_color(&self.colors.border_focused)
    }

    pub fn border_unfocused(&self) -> Color {
        Self::parse_color(&self.colors.border_unfocused)
    }

    // --- Predefined styles ---

    /// Default text style
    pub fn text(&self) -> Style {
        Style::default().fg(self.fg_main()).bg(self.bg_main())
    }

    /// Dimmed text
    pub fn text_dim(&self) -> Style {
        Style::default().fg(self.fg_dim())
    }

    /// Bright/highlighted text
    pub fn text_bright(&self) -> Style {
        Style::default().fg(self.fg_bright())
    }

    /// Title style
    pub fn title(&self) -> Style {
        Style::default()
            .fg(self.accent_primary())
            .add_modifier(Modifier::BOLD)
    }

    /// Selected item style
    pub fn selected(&self) -> Style {
        Style::default().bg(self.bg_selected()).fg(self.fg_bright())
    }

    /// Highlighted item (hover)
    pub fn highlight(&self) -> Style {
        Style::default().bg(self.bg_highlight()).fg(self.fg_main())
    }

    /// Success text
    pub fn success(&self) -> Style {
        Style::default().fg(self.accent_success())
    }

    /// Warning text
    pub fn warning(&self) -> Style {
        Style::default().fg(self.accent_warning())
    }

    /// Error text
    pub fn error(&self) -> Style {
        Style::default().fg(self.accent_error())
    }

    /// Info text
    pub fn info(&self) -> Style {
        Style::default().fg(self.accent_info())
    }

    /// Focused border
    pub fn border_focus(&self) -> Style {
        Style::default().fg(self.border_focused())
    }

    /// Unfocused border
    pub fn border_normal(&self) -> Style {
        Style::default().fg(self.border_unfocused())
    }

    /// Key hint style (for showing shortcuts)
    pub fn key_hint(&self) -> Style {
        Style::default()
            .fg(self.accent_info())
            .add_modifier(Modifier::BOLD)
    }

    /// Status bar style
    pub fn status_bar(&self) -> Style {
        Style::default().bg(self.bg_panel()).fg(self.fg_main())
    }

    /// Popup border style
    pub fn popup_border(&self) -> Style {
        Style::default().fg(self.accent_primary())
    }

    /// Progress bar style
    pub fn progress_bar(&self) -> Style {
        Style::default()
            .fg(self.accent_secondary())
            .bg(self.bg_highlight())
    }
}

/// Gruvbox Dark theme
pub fn gruvbox_dark() -> Theme {
    Theme {
        name: "gruvbox-dark".to_string(),
        colors: ThemeColors {
            bg_main: "#282828".to_string(),
            bg_panel: "#3c3836".to_string(),
            bg_highlight: "#504945".to_string(),
            bg_selected: "#665c54".to_string(),

            fg_main: "#ebdbb2".to_string(),
            fg_dim: "#928374".to_string(),
            fg_bright: "#fbf1c7".to_string(),

            accent_primary: "#83a598".to_string(),
            accent_secondary: "#d3869b".to_string(),
            accent_success: "#b8bb26".to_string(),
            accent_warning: "#fabd2f".to_string(),
            accent_error: "#fb4934".to_string(),
            accent_info: "#8ec07c".to_string(),

            graph_1: "#83a598".to_string(),
            graph_2: "#d3869b".to_string(),
            graph_3: "#8ec07c".to_string(),
            graph_4: "#fe8019".to_string(),

            border_focused: "#83a598".to_string(),
            border_unfocused: "#504945".to_string(),
        },
    }
}

/// Dracula theme
pub fn dracula() -> Theme {
    Theme {
        name: "dracula".to_string(),
        colors: ThemeColors {
            bg_main: "#282a36".to_string(),
            bg_panel: "#44475a".to_string(),
            bg_highlight: "#6272a4".to_string(),
            bg_selected: "#44475a".to_string(),

            fg_main: "#f8f8f2".to_string(),
            fg_dim: "#6272a4".to_string(),
            fg_bright: "#ffffff".to_string(),

            accent_primary: "#bd93f9".to_string(),
            accent_secondary: "#ff79c6".to_string(),
            accent_success: "#50fa7b".to_string(),
            accent_warning: "#ffb86c".to_string(),
            accent_error: "#ff5555".to_string(),
            accent_info: "#8be9fd".to_string(),

            graph_1: "#bd93f9".to_string(),
            graph_2: "#ff79c6".to_string(),
            graph_3: "#8be9fd".to_string(),
            graph_4: "#50fa7b".to_string(),

            border_focused: "#bd93f9".to_string(),
            border_unfocused: "#6272a4".to_string(),
        },
    }
}

/// Nord theme
pub fn nord() -> Theme {
    Theme {
        name: "nord".to_string(),
        colors: ThemeColors {
            bg_main: "#2e3440".to_string(),
            bg_panel: "#3b4252".to_string(),
            bg_highlight: "#434c5e".to_string(),
            bg_selected: "#4c566a".to_string(),

            fg_main: "#d8dee9".to_string(),
            fg_dim: "#4c566a".to_string(),
            fg_bright: "#eceff4".to_string(),

            accent_primary: "#88c0d0".to_string(),
            accent_secondary: "#b48ead".to_string(),
            accent_success: "#a3be8c".to_string(),
            accent_warning: "#ebcb8b".to_string(),
            accent_error: "#bf616a".to_string(),
            accent_info: "#81a1c1".to_string(),

            graph_1: "#88c0d0".to_string(),
            graph_2: "#81a1c1".to_string(),
            graph_3: "#5e81ac".to_string(),
            graph_4: "#a3be8c".to_string(),

            border_focused: "#88c0d0".to_string(),
            border_unfocused: "#4c566a".to_string(),
        },
    }
}
