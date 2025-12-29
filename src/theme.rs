use eframe::egui::{self, Color32};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::settings::Settings;

// Design system constants
// VS Code design system uses a 4px grid for spacing
pub const GRID_UNIT: f32 = 4.0;
pub const SPACING_SMALL: f32 = GRID_UNIT; // 4px
pub const SPACING_MEDIUM: f32 = 2.0 * GRID_UNIT; // 8px
pub const SPACING_LARGE: f32 = 4.0 * GRID_UNIT; // 16px
pub const TREE_INDENT: f32 = SPACING_LARGE; // 16px per tree level
pub const ROW_HEIGHT: f32 = 22.0; // VS Code row height for data rows

/// Theme color customization - only includes colors actually used in the app
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    // UI Framework colors (backgrounds, surfaces)
    pub base: String,     // Main background
    pub mantle: String,   // Secondary panels
    pub crust: String,    // Status bar background
    pub surface0: String, // Widget backgrounds
    pub surface1: String, // Widget hover/selected
    pub surface2: String, // Widget active
    pub text: String,     // Text color
    pub overlay1: String, // Catppuccin overlay1 accent

    // Syntax highlighting colors (JSON viewer)
    pub key: String,     // JSON keys/properties
    pub string: String,  // JSON string values
    pub number: String,  // JSON numbers
    pub boolean: String, // JSON booleans
    pub bracket: String, // JSON brackets/punctuation

    // Status indicator colors
    pub success: String, // Ready/success states
    pub warning: String, // Loading/warning states
    pub error: String,   // Error states
    pub info: String,    // Info/searching states

    // Sidebar-specific colors
    pub sidebar_hover: String,  // Sidebar icon hover background
    pub sidebar_header: String, // Sidebar section header text

    // Tree viewer colors
    pub indent_guide: String, // Indent guide lines in tree view

    // Selection / highlight colors
    pub selection_stroke: String,
}

impl Default for Theme {
    fn default() -> Self {
        // Default to Catppuccin Mocha (dark theme)
        Self {
            // UI Framework
            base: "#1e1e2e".to_string(),
            mantle: "#181825".to_string(),
            crust: "#11111b".to_string(),
            surface0: "#313244".to_string(),
            surface1: "#45475a".to_string(),
            surface2: "#585b70".to_string(),
            text: "#cdd6f4".to_string(),
            overlay1: "#7f849c".to_string(),
            // Syntax highlighting
            key: "#89b4fa".to_string(),     // Blue
            string: "#a6e3a1".to_string(),  // Green
            number: "#fab387".to_string(),  // Peach
            boolean: "#cba6f7".to_string(), // Mauve
            bracket: "#9399b2".to_string(), // Overlay2
            // Status indicators
            success: "#a6e3a1".to_string(), // Green
            warning: "#f9e2af".to_string(), // Yellow
            error: "#f38ba8".to_string(),   // Red
            info: "#74c7ec".to_string(),    // Sapphire
            // Sidebar
            sidebar_hover: "#6c708633".to_string(), // Overlay0 with transparency
            sidebar_header: "#9399b2".to_string(),  // Overlay2
            // Tree viewer
            indent_guide: "#45475a".to_string(), // Surface1
            // Selection (Catppuccin lavender accent)
            selection_stroke: "#89b4fa".to_string(),
        }
    }
}

impl Theme {
    /// Create a theme based on dark mode setting
    pub fn for_dark_mode(dark_mode: bool) -> Self {
        if dark_mode {
            Self::default() // Catppuccin Mocha
        } else {
            Self::catppuccin_latte()
        }
    }

    /// Create a Catppuccin Latte (light) theme
    pub fn catppuccin_latte() -> Self {
        Self {
            // UI Framework
            base: "#eff1f5".to_string(),
            mantle: "#e6e9ef".to_string(),
            crust: "#dce0e8".to_string(),
            surface0: "#ccd0da".to_string(),
            surface1: "#bcc0cc".to_string(),
            surface2: "#acb0be".to_string(),
            text: "#4c4f69".to_string(),
            overlay1: "#8c8fa1".to_string(),
            // Syntax highlighting
            key: "#1e66f5".to_string(),     // Blue
            string: "#40a02b".to_string(),  // Green
            number: "#fe640b".to_string(),  // Peach
            boolean: "#8839ef".to_string(), // Mauve
            bracket: "#7c7f93".to_string(), // Overlay2
            // Status indicators
            success: "#40a02b".to_string(), // Green
            warning: "#df8e1d".to_string(), // Yellow
            error: "#d20f39".to_string(),   // Red
            info: "#209fb5".to_string(),    // Sapphire
            // Sidebar
            sidebar_hover: "#9ca0b033".to_string(), // Overlay0 with transparency
            sidebar_header: "#7c7f93".to_string(),  // Overlay2
            // Tree viewer
            indent_guide: "#bcc0cc".to_string(), // Surface1
            selection_stroke: "#1e66f5".to_string(),
        }
    }

    /// Parse a hex color string (e.g., "#1e1e2e" or "1e1e2e") into Color32
    fn parse_color(hex: &str) -> Color32 {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            eprintln!("Invalid color format: {}, using black", hex);
            return Color32::BLACK;
        }

        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);

        Color32::from_rgb(r, g, b)
    }

    /// Parse a hex color string with alpha (e.g., "#1e1e2e33" or "1e1e2e33") into Color32
    fn parse_color_with_alpha(hex: &str) -> Color32 {
        let hex = hex.trim_start_matches('#');
        if hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            Color32::from_rgba_unmultiplied(r, g, b, a)
        } else if hex.len() == 6 {
            // Fallback to opaque color if no alpha provided
            Self::parse_color(hex)
        } else {
            eprintln!("Invalid color format: {}, using black", hex);
            Color32::BLACK
        }
    }

    /// Get parsed Color32 values from hex strings
    pub fn colors(&self) -> ThemeColors {
        ThemeColors {
            base: Self::parse_color(&self.base),
            mantle: Self::parse_color(&self.mantle),
            crust: Self::parse_color(&self.crust),
            surface0: Self::parse_color(&self.surface0),
            surface1: Self::parse_color(&self.surface1),
            surface2: Self::parse_color(&self.surface2),
            text: Self::parse_color(&self.text),
            overlay1: Self::parse_color(&self.overlay1),
            key: Self::parse_color(&self.key),
            string: Self::parse_color(&self.string),
            number: Self::parse_color(&self.number),
            boolean: Self::parse_color(&self.boolean),
            bracket: Self::parse_color(&self.bracket),
            success: Self::parse_color(&self.success),
            warning: Self::parse_color(&self.warning),
            error: Self::parse_color(&self.error),
            info: Self::parse_color(&self.info),
            sidebar_hover: Self::parse_color_with_alpha(&self.sidebar_hover),
            sidebar_header: Self::parse_color(&self.sidebar_header),
            indent_guide: Self::parse_color(&self.indent_guide),
            selection_stroke: Self::parse_color(&self.selection_stroke),
        }
    }
}

/// Parsed Color32 values from Theme hex strings
#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    pub base: Color32,
    pub mantle: Color32,
    pub crust: Color32,
    pub surface0: Color32,
    pub surface1: Color32,
    pub surface2: Color32,
    pub text: Color32,
    pub overlay1: Color32,
    pub key: Color32,
    pub string: Color32,
    pub number: Color32,
    pub boolean: Color32,
    pub bracket: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub info: Color32,
    pub sidebar_hover: Color32,
    pub sidebar_header: Color32,
    pub indent_guide: Color32,
    pub selection_stroke: Color32,
}

/// Apply theme settings including visuals and fonts
pub fn apply_theme(ctx: &egui::Context, settings: &Settings) {
    // Get theme colors based on dark_mode setting
    // Users can configure custom themes (including high contrast) via settings.theme
    let theme = Theme::for_dark_mode(settings.dark_mode);
    let theme_colors = theme.colors();

    // Store theme colors in egui memory for access throughout the app
    ctx.memory_mut(|mem| {
        mem.data
            .insert_temp(egui::Id::new("theme_colors"), theme_colors);
    });

    // Apply visual theme with custom colors
    ctx.set_visuals(create_visuals(settings.dark_mode, &theme_colors));

    // Set system theme for native title bar (macOS traffic lights, Windows title bar, etc.)
    let system_theme = if settings.dark_mode {
        egui::viewport::SystemTheme::Dark
    } else {
        egui::viewport::SystemTheme::Light
    };
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(system_theme));

    // Apply style settings (spacing, fonts, etc.)
    let mut style = (*ctx.style()).clone();

    // Spacing: VS Code design system uses 4px grid
    style.spacing.item_spacing = egui::vec2(SPACING_MEDIUM, SPACING_SMALL);
    style.spacing.button_padding = egui::vec2(SPACING_MEDIUM, SPACING_SMALL);
    style.spacing.indent = TREE_INDENT;

    // Apply font sizes
    let font_size = settings.font_size;
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::proportional(font_size * 0.85),
    );
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(font_size));
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::proportional(font_size),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(font_size * 1.2),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::monospace(font_size),
    );

    // Apply animation settings
    if !settings.ui.enable_animations {
        style.animation_time = 0.0;
    }

    ctx.set_style(style);
}

/// Create egui visuals from theme colors
fn create_visuals(dark_mode: bool, colors: &ThemeColors) -> egui::Visuals {
    let mut v = if dark_mode {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    // Apply theme colors
    v.override_text_color = Some(colors.text);
    v.panel_fill = colors.base; // Main background
    v.extreme_bg_color = colors.mantle; // Panels/cards

    // Use a very subtle color for alternating rows (lower contrast than surface0)
    // Mix base with surface0 at 30% opacity for subtle effect
    v.faint_bg_color = if dark_mode {
        Color32::from_rgba_unmultiplied(49, 50, 68, 77) // surface0 at 30% opacity over base
    } else {
        Color32::from_rgba_unmultiplied(204, 208, 218, 77) // surface0 at 30% opacity over base
    };

    // Widget colors
    v.widgets.noninteractive.bg_fill = colors.surface0;
    v.widgets.inactive.bg_fill = colors.surface0;
    v.widgets.hovered.bg_fill = colors.surface1;
    v.widgets.active.bg_fill = colors.surface2;
    v.widgets.active.fg_stroke.color = if dark_mode { colors.base } else { colors.text };

    // Selection colors derived from theme palette
    v.selection.bg_fill = colors.overlay1;
    v.selection.stroke = egui::Stroke::new(1.0, colors.selection_stroke);
    v.hyperlink_color = colors.key;

    v
}

pub fn row_fill(i: usize, ui: &egui::Ui) -> Color32 {
    if i % 2 == 1 {
        // Only paint odd rows - use faint_bg_color which we set from theme.surface0
        ui.visuals().faint_bg_color
    } else {
        Color32::TRANSPARENT // even rows = "no fill"
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TextToken {
    Key,
    Str,
    Number,
    Boolean,
    Bracket,
}

impl From<&Value> for TextToken {
    fn from(value: &Value) -> Self {
        match value {
            Value::String(_) => TextToken::Str,
            Value::Number(_) => TextToken::Number,
            Value::Bool(_) => TextToken::Boolean,
            Value::Array(_) => TextToken::Bracket,
            Value::Object(_) => TextToken::Key,
            Value::Null => TextToken::Boolean,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TextPalette {
    pub key: Color32,
    pub string: Color32,
    pub number: Color32,
    pub boolean: Color32,
    pub bracket: Color32,
}

impl TextPalette {
    /// Get TextPalette from custom theme colors stored in egui memory
    pub fn from_context(ctx: &egui::Context) -> Self {
        ctx.memory(|mem| {
            let theme_colors = mem
                .data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| {
                    // Fallback: create default theme based on dark mode from visuals
                    let dark_mode = ctx.style().visuals.dark_mode;
                    Theme::for_dark_mode(dark_mode).colors()
                });

            Self {
                key: theme_colors.key,
                string: theme_colors.string,
                number: theme_colors.number,
                boolean: theme_colors.boolean,
                bracket: theme_colors.bracket,
            }
        })
    }

    pub fn color(&self, token: TextToken) -> Color32 {
        match token {
            TextToken::Key => self.key,
            TextToken::Str => self.string,
            TextToken::Number => self.number,
            TextToken::Boolean => self.boolean,
            TextToken::Bracket => self.bracket,
        }
    }

    /// Get color for a token, or base text color if syntax highlighting is disabled
    pub fn color_with_highlighting(
        &self,
        token: TextToken,
        syntax_highlighting: bool,
        base_color: Color32,
    ) -> Color32 {
        if syntax_highlighting {
            self.color(token)
        } else {
            base_color
        }
    }
}

pub fn selected_row_bg(ui: &egui::Ui) -> Color32 {
    // Use widget active state color which we set from theme.surface2
    ui.visuals().widgets.active.bg_fill
}

/// Hover overlay for rows
pub fn hover_row_bg(ui: &egui::Ui) -> Color32 {
    // Use widget hovered state color which we set from theme.surface1
    ui.visuals().widgets.hovered.bg_fill
}
