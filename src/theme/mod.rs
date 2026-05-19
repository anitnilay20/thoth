mod constants;

pub use constants::*;
use eframe::egui::{self, Color32};
use egui_code_editor::ColorTheme;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    plugin::theme_plugin::{get_plugin_theme_by_name, get_plugin_theme_catalog},
    settings::Settings,
};

// ── Design-system constants ───────────────────────────────────────────────────

// ── Theme (serialisable hex-string form) ──────────────────────────────────────

/// All colours used by Thoth, named by **role** rather than palette position.
///
/// This is the serialised form stored in settings (hex strings). Call
/// `.colors()` to get parsed [`ThemeColors`] for use in rendering.
///
/// Groups follow egui's `Visuals` conventions:
///   • `bg_*`     – layered background depths
///   • `surface_*`– interactive widget fills
///   • `fg_*`     – foreground / text
///   • `syntax_*` – code / JSON syntax highlighting
///   • `status_*` – semantic status indicators
///   • `accent_*` – brand / interactive highlights
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

    /// Returns parsed [`ThemeColors`] (all `Color32` values) from this theme.
    pub fn colors(&self) -> ThemeColors {
        ThemeColors {
            bg: Self::parse_color(&self.bg),
            bg_panel: Self::parse_color(&self.bg_panel),
            bg_sunken: Self::parse_color(&self.bg_sunken),

            surface: Self::parse_color(&self.surface),
            surface_raised: Self::parse_color(&self.surface_raised),
            surface_active: Self::parse_color(&self.surface_active),

            fg: Self::parse_color(&self.fg),
            fg_muted: Self::parse_color(&self.fg_muted),

            syntax_key: Self::parse_color(&self.syntax_key),
            syntax_string: Self::parse_color(&self.syntax_string),
            syntax_number: Self::parse_color(&self.syntax_number),
            syntax_bool: Self::parse_color(&self.syntax_bool),
            syntax_punctuation: Self::parse_color(&self.syntax_punctuation),

            success: Self::parse_color(&self.success),
            warning: Self::parse_color(&self.warning),
            error: Self::parse_color(&self.error),
            info: Self::parse_color(&self.info),

            accent: Self::parse_color(&self.accent),
            accent_secondary: Self::parse_color(&self.accent_secondary),

            sidebar_hover: Self::parse_color_with_alpha(&self.sidebar_hover),
            sidebar_header: Self::parse_color(&self.sidebar_header),
            indent_guide: Self::parse_color(&self.indent_guide),
        }
    }

    pub fn parse_color(hex: &str) -> Color32 {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return Color32::BLACK;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        Color32::from_rgb(r, g, b)
    }

    fn parse_color_with_alpha(hex: &str) -> Color32 {
        let hex = hex.trim_start_matches('#');
        if hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            Color32::from_rgba_unmultiplied(r, g, b, a)
        } else {
            Self::parse_color(hex)
        }
    }
}

// ── ThemeColors (parsed Color32 form) ────────────────────────────────────────

/// Parsed `Color32` values ready for use in egui rendering.
///
/// Obtain via [`Theme::colors()`] or read from egui memory with key
/// `egui::Id::new("theme_colors")`.
#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    // Backgrounds
    pub bg: Color32,
    pub bg_panel: Color32,
    pub bg_sunken: Color32,

    // Surfaces
    pub surface: Color32,
    pub surface_raised: Color32,
    pub surface_active: Color32,

    // Foreground
    pub fg: Color32,
    pub fg_muted: Color32,

    // Syntax
    pub syntax_key: Color32,
    pub syntax_string: Color32,
    pub syntax_number: Color32,
    pub syntax_bool: Color32,
    pub syntax_punctuation: Color32,

    // Status
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub info: Color32,

    // Accents
    pub accent: Color32,
    pub accent_secondary: Color32,

    // Component-specific
    pub sidebar_hover: Color32,
    pub sidebar_header: Color32,
    pub indent_guide: Color32,
}

impl ThemeColors {
    /// Build an `egui_code_editor` `ColorTheme` from the current palette.
    ///
    /// Called per-frame from `render_ui_node`; `hex()` interns each unique
    /// colour in a static cache to avoid per-frame allocations.
    pub fn code_editor_theme(&self) -> ColorTheme {
        fn hex(c: Color32) -> &'static str {
            use std::collections::HashMap;
            use std::sync::Mutex;
            static CACHE: std::sync::OnceLock<Mutex<HashMap<u32, &'static str>>> =
                std::sync::OnceLock::new();
            let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
            let key = ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | (c.b() as u32);
            let mut map = cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(&s) = map.get(&key) {
                return s;
            }
            let s = Box::leak(format!("{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()).into_boxed_str());
            map.insert(key, s);
            s
        }
        let is_dark = get_contrast_text_color(self.bg) == Color32::WHITE;
        let fg_hex = hex(self.fg);
        let string_hex = hex(self.syntax_string);
        ColorTheme {
            name: if is_dark { "Thoth Dark" } else { "Thoth Light" },
            dark: is_dark,
            bg: hex(self.bg_panel),
            cursor: fg_hex,
            selection: hex(self.surface_raised),
            comments: hex(self.fg_muted),
            functions: hex(self.syntax_key),
            keywords: hex(self.accent),
            literals: string_hex,
            numerics: hex(self.syntax_number),
            punctuation: fg_hex,
            strs: string_hex,
            types: hex(self.info),
            special: hex(self.error),
        }
    }
}

// ── apply_fonts ───────────────────────────────────────────────────────────────

/// Rebuild and apply `FontDefinitions` only when `font_family` changes.
/// Comparing against a cached value in egui memory prevents the expensive
/// re-rasterise from running every frame.
pub fn apply_fonts(ctx: &egui::Context, settings: &Settings) {
    let cache_key = egui::Id::new("applied_font_family");
    let last: Option<Option<String>> = ctx.memory(|m| m.data.get_temp(cache_key));

    // `last == None` means never applied yet — always run on first call.
    if last.as_ref().map(|v| v.as_deref()) == Some(settings.font_family.as_deref()) {
        return;
    }

    let mut fonts = egui::FontDefinitions::default();

    // Register Phosphor first so its font data is available, then expose it as
    // a dedicated named family. Icon widgets use FontFamily::Name("phosphor")
    // directly instead of relying on fallback order in Proportional.
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    fonts.families.insert(
        egui::FontFamily::Name("phosphor".into()),
        vec!["phosphor".into()],
    );

    if let Some(family) = &settings.font_family
        && let Some(bytes) = crate::platform::find_font_bytes(family) {
            fonts.font_data.insert(
                family.clone(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );

            // Monospace-style fonts: prepend to both Proportional and Monospace stacks.
            // Purely proportional fonts (e.g. Inter) only go into Proportional.
            let is_mono = !matches!(family.as_str(), "Inter");
            for target in [egui::FontFamily::Proportional]
                .iter()
                .chain(is_mono.then_some(&egui::FontFamily::Monospace))
            {
                if let Some(list) = fonts.families.get_mut(target) {
                    list.insert(0, family.clone());
                }
            }
        }

    ctx.set_fonts(fonts);
    ctx.memory_mut(|m| m.data.insert_temp(cache_key, settings.font_family.clone()));
}

/// Returns a [`egui::FontId`] that always resolves to the Phosphor icon font,
/// bypassing the Proportional fallback chain entirely.
pub fn phosphor_font_id(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("phosphor".into()))
}

/// Returns a [`egui::RichText`] pre-configured with the Phosphor icon font.
/// Use this for any inline icon rendered as a label or inside a widget.
pub fn icon_rich_text(icon: &str, size: f32) -> egui::RichText {
    egui::RichText::new(icon).font(phosphor_font_id(size))
}

// ── apply_theme ───────────────────────────────────────────────────────────────

pub fn apply_theme(ctx: &egui::Context, settings: &Settings) {
    apply_fonts(ctx, settings);

    let theme = &settings.theme;
    let is_dark = theme.dark_mode;
    let colors = theme.colors();

    ctx.memory_mut(|mem| {
        mem.data.insert_temp(egui::Id::new("theme_colors"), colors);
    });

    ctx.set_visuals(build_visuals(is_dark, &colors));

    let system_theme = if is_dark {
        egui::viewport::SystemTheme::Dark
    } else {
        egui::viewport::SystemTheme::Light
    };
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(system_theme));

    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(SPACING_MEDIUM, SPACING_SMALL);
    style.spacing.button_padding = egui::vec2(SPACING_MEDIUM, SPACING_SMALL);
    style.spacing.indent = TREE_INDENT;

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

    if !settings.ui.enable_animations {
        style.animation_time = 0.0;
    }
    ctx.set_global_style(style);
}

/// Build [`egui::Visuals`] from [`ThemeColors`].
///
/// We override every colour egui might render so no dark/light default bleeds
/// through — especially important for light themes applied on top of the
/// `Visuals::light()` base which still carries different defaults than ours.
fn build_visuals(dark_mode: bool, c: &ThemeColors) -> egui::Visuals {
    let mut v = if dark_mode {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    // ── Backgrounds ──────────────────────────────────────────────────────────
    v.override_text_color = Some(c.fg);
    v.panel_fill = c.bg;
    v.window_fill = c.bg_panel; // modal / popup backgrounds
    v.extreme_bg_color = c.surface; // TextEdit / ComboBox text-area background
    v.code_bg_color = c.surface; // inline code spans
    v.faint_bg_color =
        Color32::from_rgba_unmultiplied(c.surface.r(), c.surface.g(), c.surface.b(), 77);

    // ── Widget fills ─────────────────────────────────────────────────────────
    // noninteractive = labels, separators (use surface so read-only fields match)
    // inactive        = resting inputs, buttons, selects
    // hovered         = cursor-over state
    // active          = pressed / being edited
    // open            = ComboBox dropdown open
    v.widgets.noninteractive.bg_fill = c.surface;
    v.widgets.inactive.bg_fill = c.surface;
    v.widgets.hovered.bg_fill = c.surface_raised;
    v.widgets.active.bg_fill = c.surface_active;
    v.widgets.open.bg_fill = c.surface_raised;

    // ── Widget text / stroke ─────────────────────────────────────────────────
    // egui uses fg_stroke to paint text inside labels, buttons, and inputs.
    // Not overriding these is the main reason dark text appears on light themes.
    let fg_stroke = egui::Stroke::new(1.0, c.fg);
    let muted_stroke = egui::Stroke::new(1.0, c.fg_muted);
    v.widgets.noninteractive.fg_stroke = muted_stroke;
    v.widgets.inactive.fg_stroke = fg_stroke;
    v.widgets.hovered.fg_stroke = fg_stroke;
    v.widgets.active.fg_stroke = egui::Stroke::new(1.5, c.fg);
    v.widgets.open.fg_stroke = fg_stroke;

    // ── Widget borders ───────────────────────────────────────────────────────
    let border = egui::Stroke::new(1.0, c.surface_raised);
    v.widgets.noninteractive.bg_stroke = border;
    v.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, c.accent);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0, c.accent);

    // ── Windows / popups ─────────────────────────────────────────────────────
    v.window_stroke = egui::Stroke::new(1.0, c.surface_raised);
    v.popup_shadow = egui::Shadow::NONE;
    v.window_shadow = egui::Shadow::NONE;

    // ── Selection & links ────────────────────────────────────────────────────
    v.selection.bg_fill =
        Color32::from_rgba_unmultiplied(c.accent.r(), c.accent.g(), c.accent.b(), 60);
    v.selection.stroke = egui::Stroke::new(1.0, c.accent);
    v.hyperlink_color = c.syntax_key;

    // ── Text cursor ──────────────────────────────────────────────────────────
    v.text_cursor.stroke = egui::Stroke::new(2.0, c.accent);

    v
}

// ── Row helpers ───────────────────────────────────────────────────────────────

pub fn row_fill(i: usize, ui: &egui::Ui) -> Color32 {
    if i % 2 == 1 {
        ui.visuals().faint_bg_color
    } else {
        Color32::TRANSPARENT
    }
}

pub fn selected_row_bg(ui: &egui::Ui) -> Color32 {
    ui.visuals().widgets.active.bg_fill
}

pub fn hover_row_bg(ui: &egui::Ui) -> Color32 {
    ui.visuals().widgets.hovered.bg_fill
}

// ── Contrast helper ───────────────────────────────────────────────────────────

/// Return `WHITE` when the background is dark, `BLACK` when it is light.
/// Uses the WCAG 2.0 relative luminance formula.
pub fn get_contrast_text_color(bg: Color32) -> Color32 {
    fn linearise(c: f32) -> f32 {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    let r = linearise(bg.r() as f32 / 255.0);
    let g = linearise(bg.g() as f32 / 255.0);
    let b = linearise(bg.b() as f32 / 255.0);
    let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let w = (1.0 + 0.05) / (lum + 0.05);
    let k = (lum + 0.05) / 0.05;
    if w >= k {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

// ── Syntax token helpers ──────────────────────────────────────────────────────

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
            Value::String(_) => Self::Str,
            Value::Number(_) => Self::Number,
            Value::Bool(_) => Self::Boolean,
            Value::Array(_) | Value::Object(_) => Self::Bracket,
            Value::Null => Self::Boolean,
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
    pub fn from_context(ctx: &egui::Context) -> Self {
        ctx.memory(|mem| {
            let tc = mem
                .data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| {
                    Theme::for_dark_mode(ctx.global_style().visuals.dark_mode).colors()
                });
            Self {
                key: tc.syntax_key,
                string: tc.syntax_string,
                number: tc.syntax_number,
                boolean: tc.syntax_bool,
                bracket: tc.syntax_punctuation,
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

    pub fn color_with_highlighting(
        &self,
        token: TextToken,
        enabled: bool,
        base: Color32,
    ) -> Color32 {
        if enabled { self.color(token) } else { base }
    }
}

// ── BgColorOptions ────────────────────────────────────────────────────────────

/// Named background-colour slots usable in the plugin DSL `bg-color` field.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum BgColorOptions {
    #[default]
    None,
    /// `bg` — main app background.
    Bg,
    /// `bg-panel` — sidebar / secondary panels.
    BgPanel,
    /// `bg-sunken` — status bar / deepest inset.
    BgSunken,
    /// `surface` — resting widget fill.
    Surface,
    /// `surface-raised` — hover / elevated widget fill.
    SurfaceRaised,
    /// `surface-active` — pressed / active widget fill.
    SurfaceActive,
    /// `fg-muted` — muted foreground (useful for subtle dividers).
    FgMuted,
    // Legacy aliases kept for backwards-compatibility with existing plugin JSON.
    /// Alias for `bg`.
    Base,
    /// Alias for `bg-panel`.
    Mantle,
    /// Alias for `bg-sunken`.
    Crust,
    /// Alias for `surface`.
    Surface0,
    /// Alias for `surface-raised`.
    Surface1,
    /// Alias for `surface-active`.
    Surface2,
    /// Alias for `fg-muted`.
    Overlay1,
}

impl BgColorOptions {
    pub fn into_color(self, c: &ThemeColors) -> Option<Color32> {
        match self {
            Self::None => None,
            Self::Bg | Self::Base => Some(c.bg),
            Self::BgPanel | Self::Mantle => Some(c.bg_panel),
            Self::BgSunken | Self::Crust => Some(c.bg_sunken),
            Self::Surface | Self::Surface0 => Some(c.surface),
            Self::SurfaceRaised | Self::Surface1 => Some(c.surface_raised),
            Self::SurfaceActive | Self::Surface2 => Some(c.surface_active),
            Self::FgMuted | Self::Overlay1 => Some(c.fg_muted),
        }
    }
}
