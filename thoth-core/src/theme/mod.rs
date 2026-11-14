//! Theme data, in its serialisable hex-string form.
//!
//! This is the display-independent half of the theme: the serialised form
//! stored in settings, the built-in palettes, and plugin theme discovery.
//! `ThemeColors` (parsed `Color32`) and everything that applies a theme to an
//! `egui::Context` live in `thoth-ui`.

mod constants;
pub use constants::*;

use serde::{Deserialize, Serialize};

use crate::plugin::theme_plugin::{get_plugin_theme_by_name, get_plugin_theme_catalog};

/// All colours used by Thoth, named by **role** rather than palette position.
///
/// This is the serialised form stored in settings (hex strings). Call
/// `.colors()` in `thoth-ui` to get parsed `ThemeColors` for use in rendering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Theme {
    pub name: String,
    pub dark_mode: bool,

    // ── Backgrounds (deepest → shallowest) ──────────────────────────────────
    /// Main app background.
    pub bg: String,
    /// Secondary panels — sidebar, cards, drawers.
    pub bg_panel: String,
    /// Deepest background — status bar, inset areas.
    pub bg_sunken: String,

    // ── Surfaces (widget fills by interaction state) ─────────────────────────
    /// Resting widget fill — inputs, dropdowns, list rows.
    pub surface: String,
    /// Elevated / hovered widget fill.
    pub surface_raised: String,
    /// Pressed / active widget fill.
    pub surface_active: String,

    // ── Foreground / text ────────────────────────────────────────────────────
    /// Primary text colour.
    pub fg: String,
    /// Muted / secondary text — placeholders, captions, disabled labels.
    pub fg_muted: String,

    // ── Syntax (JSON / code viewer) ──────────────────────────────────────────
    pub syntax_key: String,
    pub syntax_string: String,
    pub syntax_number: String,
    pub syntax_bool: String,
    pub syntax_punctuation: String,

    // ── Status indicators ────────────────────────────────────────────────────
    pub success: String,
    pub warning: String,
    pub error: String,
    pub info: String,

    // ── Accents ──────────────────────────────────────────────────────────────
    /// Primary accent — links, active borders, spinners, selection stroke.
    pub accent: String,
    /// Secondary accent — badges, complementary highlights.
    pub accent_secondary: String,

    // ── Component-specific slots ─────────────────────────────────────────────
    /// Sidebar icon-hover background (may include alpha, e.g. `"#6c708633"`).
    pub sidebar_hover: String,
    /// Sidebar section-header label colour.
    pub sidebar_header: String,
    /// Tree-view indent guide lines.
    pub indent_guide: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self::mocha()
    }
}

impl Theme {
    pub fn for_dark_mode(dark_mode: bool) -> Self {
        if dark_mode {
            Self::default()
        } else {
            Self::latte()
        }
    }

    /// All themes as `(display_name, is_dark, family)` tuples.
    pub fn catalog() -> Vec<(String, bool, String)> {
        let theme_plugins_catalog = get_plugin_theme_catalog();
        [
            vec![
                (
                    "Catppuccin Mocha".to_string(),
                    true,
                    "Catppuccin".to_string(),
                ),
                (
                    "Catppuccin Latte".to_string(),
                    false,
                    "Catppuccin".to_string(),
                ),
                (
                    "Catppuccin Frappé".to_string(),
                    true,
                    "Catppuccin".to_string(),
                ),
                (
                    "Catppuccin Macchiato".to_string(),
                    true,
                    "Catppuccin".to_string(),
                ),
                ("Dracula".to_string(), true, "Classic".to_string()),
                ("Nord".to_string(), true, "Classic".to_string()),
                ("Gruvbox Dark".to_string(), true, "Classic".to_string()),
                ("Tokyo Night".to_string(), true, "Modern".to_string()),
                ("Rosé Pine".to_string(), true, "Modern".to_string()),
                ("GitHub Light".to_string(), false, "Modern".to_string()),
                ("Solarized Dark".to_string(), true, "Solarized".to_string()),
                (
                    "Solarized Light".to_string(),
                    false,
                    "Solarized".to_string(),
                ),
            ],
            theme_plugins_catalog,
        ]
        .concat()
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "Catppuccin Mocha" => Self::mocha(),
            "Catppuccin Latte" => Self::latte(),
            "Catppuccin Frappé" => Self::frappe(),
            "Catppuccin Macchiato" => Self::macchiato(),
            "Dracula" => Self::dracula(),
            "Nord" => Self::nord(),
            "Gruvbox Dark" => Self::gruvbox_dark(),
            "Tokyo Night" => Self::tokyo_night(),
            "Rosé Pine" => Self::rose_pine(),
            "GitHub Light" => Self::github_light(),
            "Solarized Dark" => Self::solarized_dark(),
            "Solarized Light" => Self::solarized_light(),
            _ => get_plugin_theme_by_name(name).unwrap_or_default(),
        }
    }
}
