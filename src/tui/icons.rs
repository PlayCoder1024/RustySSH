//! Icon support with Nerd Font detection and fallback

/// Icon set for the UI - supports both Nerd Font and ASCII fallback
#[derive(Debug, Clone)]
pub struct Icons {
    /// Whether Nerd Fonts are available
    pub nerd_font: bool,

    // Navigation & UI
    pub keyboard: &'static str,
    pub arrow_right: &'static str,
    pub arrow_down: &'static str,
    pub check: &'static str,

    // Views
    pub connections: &'static str,
    pub terminal: &'static str,
    pub folder: &'static str,
    pub transfer: &'static str,
    pub tunnel: &'static str,
    pub key: &'static str,
    pub settings: &'static str,
    pub help: &'static str,
    pub info: &'static str,

    // Auth methods
    pub password: &'static str,
    pub key_file: &'static str,
    pub agent: &'static str,
    pub certificate: &'static str,

    // Status
    pub connected: &'static str,
    pub disconnected: &'static str,
    pub warning: &'static str,
    pub error: &'static str,
    pub success: &'static str,
}

impl Icons {
    /// Create icons with Nerd Font glyphs
    pub fn nerd_font() -> Self {
        Self {
            nerd_font: true,
            keyboard: "󰌑 ",
            arrow_right: "󰅂 ",
            arrow_down: "󰅀 ",
            check: " ",
            connections: "󰢹 ",
            terminal: "󰆍 ",
            folder: "󰉋 ",
            transfer: "󰇚 ",
            tunnel: "󰛳 ",
            key: "󰌋 ",
            settings: "󰒓 ",
            help: "󰋖 ",
            info: "󰋼 ",
            password: "󰌆 ",
            key_file: "󰌋 ",
            agent: "󰌉 ",
            certificate: "󰄤 ",
            connected: "● ",
            disconnected: "○ ",
            warning: " ",
            error: " ",
            success: " ",
        }
    }

    /// Create icons with ASCII/Unicode fallback
    pub fn ascii() -> Self {
        Self {
            nerd_font: false,
            keyboard: "» ",
            arrow_right: "> ",
            arrow_down: "v ",
            check: "✓ ",
            connections: "⚡ ",
            terminal: "▣ ",
            folder: "📁 ",
            transfer: "⇄ ",
            tunnel: "🔗 ",
            key: "🔑 ",
            settings: "⚙ ",
            help: "? ",
            info: "ℹ ",
            password: "🔒 ",
            key_file: "🔑 ",
            agent: "🔐 ",
            certificate: "📜 ",
            connected: "● ",
            disconnected: "○ ",
            warning: "⚠ ",
            error: "✗ ",
            success: "✓ ",
        }
    }

    /// Auto-detect based on TERM and environment
    pub fn detect() -> Self {
        // Check for environment hints that suggest Nerd Font support
        let term = std::env::var("TERM").unwrap_or_default();
        let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
        let nerd_font_env = std::env::var("NERD_FONT").unwrap_or_default();

        // Explicit environment variable override
        if nerd_font_env == "1" || nerd_font_env.to_lowercase() == "true" {
            return Self::nerd_font();
        }
        if nerd_font_env == "0" || nerd_font_env.to_lowercase() == "false" {
            return Self::ascii();
        }

        // Check for terminals commonly configured with Nerd Fonts
        // These are heuristics - users can set NERD_FONT=1 to force
        let likely_nerd_font = term_program.contains("kitty")
            || term_program.contains("Alacritty")
            || term_program.contains("WezTerm")
            || term_program.contains("Hyper")
            || term.contains("kitty")
            || std::env::var("KITTY_WINDOW_ID").is_ok();

        if likely_nerd_font {
            Self::nerd_font()
        } else {
            // Default to ASCII for maximum compatibility
            // Users can set NERD_FONT=1 if they have Nerd Fonts
            Self::ascii()
        }
    }
}

impl Default for Icons {
    fn default() -> Self {
        Self::detect()
    }
}
