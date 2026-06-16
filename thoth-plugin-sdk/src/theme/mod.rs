use egui::Color32;

pub use crate::tokens::TextToken;

/// Standard height of a tree/data row, in points.
pub const ROW_HEIGHT: f32 = 22.0;

/// egui memory key under which the host publishes the active [`ThemeColors`].
///
/// The host writes the parsed palette here once per frame (via its
/// `apply_theme`), and every SDK widget reads it back through
/// [`ThemeColors::from_ctx`]. This is the single hand-off point between the
/// application (which *owns* the theme) and the library (which only *consumes*
/// it) — keep the key in sync on both sides.
pub const THEME_MEMORY_ID: &str = "theme_colors";

/// Parsed `Color32` palette ready for use in egui rendering.
///
/// This is the canonical, library-owned palette type. The application is
/// responsible for choosing the actual colours and publishing them into egui
/// memory under [`THEME_MEMORY_ID`]; SDK widgets never pick colours themselves,
/// they only read this struct back via [`ThemeColors::from_ctx`]. Colours are
/// named by **role** (e.g. `accent`, `surface_active`) rather than palette
/// position so a single theme drives both host-native and plugin-defined UI
/// consistently.
#[derive(Debug, Clone, Copy)]
#[derive(Default)]
pub struct ThemeColors {
    // Backgrounds
    /// Main app background.
    pub bg: Color32,
    /// Secondary panels — sidebar, cards, drawers.
    pub bg_panel: Color32,
    /// Deepest background — status bar, inset areas.
    pub bg_sunken: Color32,

    // Surfaces
    /// Resting widget fill — inputs, dropdowns, list rows.
    pub surface: Color32,
    /// Elevated / hovered widget fill.
    pub surface_raised: Color32,
    /// Pressed / active widget fill.
    pub surface_active: Color32,

    // Foreground
    /// Primary text colour.
    pub fg: Color32,
    /// Muted / secondary text — placeholders, captions, disabled labels.
    pub fg_muted: Color32,

    // Syntax
    /// JSON / code key colour.
    pub syntax_key: Color32,
    /// JSON / code string-literal colour.
    pub syntax_string: Color32,
    /// JSON / code numeric-literal colour.
    pub syntax_number: Color32,
    /// JSON / code boolean / null colour.
    pub syntax_bool: Color32,
    /// JSON / code punctuation colour.
    pub syntax_punctuation: Color32,

    // Status
    /// Success status indicator.
    pub success: Color32,
    /// Warning status indicator.
    pub warning: Color32,
    /// Error status indicator.
    pub error: Color32,
    /// Informational status indicator.
    pub info: Color32,

    // Accents
    /// Primary accent — links, active borders, spinners, selection stroke.
    pub accent: Color32,
    /// Secondary accent — badges, complementary highlights.
    pub accent_secondary: Color32,

    // Component-specific
    /// Sidebar icon-hover background (may include alpha).
    pub sidebar_hover: Color32,
    /// Sidebar section-header label colour.
    pub sidebar_header: Color32,
    /// Tree-view indent guide lines.
    pub indent_guide: Color32,
}

impl ThemeColors {
    /// Read the host-injected palette from egui memory.
    ///
    /// Falls back to [`ThemeColors::default`] (a transparent/zeroed palette)
    /// when the host has not published a theme yet — in normal operation the
    /// host writes the palette every frame before any widget renders.
    pub fn from_ctx(ctx: &egui::Context) -> Self {
        ctx.memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new(THEME_MEMORY_ID))
                .unwrap_or_default()
        })
    }
}

/// Return `WHITE` when the background is dark, `BLACK` when it is light.
///
/// Uses the WCAG 2.0 relative-luminance formula to pick a legible text colour
/// for an arbitrary background fill.
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

/// Parse a `#rrggbb` or `#rrggbbaa` hex string into a [`Color32`].
///
/// Returns `None` for anything that isn't a valid 6- or 8-digit hex colour, so
/// callers can fall back to a theme role.
pub fn parse_hex_color(hex: &str) -> Option<Color32> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    match hex.len() {
        6 => Some(Color32::from_rgb(byte(0)?, byte(2)?, byte(4)?)),
        8 => Some(Color32::from_rgba_unmultiplied(
            byte(0)?,
            byte(2)?,
            byte(4)?,
            byte(6)?,
        )),
        _ => None,
    }
}

/// Background fill used for a hovered tree/data row.
pub fn hover_row_bg(ui: &egui::Ui) -> Color32 {
    ui.visuals().widgets.hovered.bg_fill
}

// ── Syntax token helpers ──────────────────────────────────────────────────────

/// Resolved syntax colours for the active theme, one per [`TextToken`].
#[derive(Clone, Copy, Debug)]
pub struct TextPalette {
    /// Colour for [`TextToken::Key`].
    pub key: Color32,
    /// Colour for [`TextToken::Str`].
    pub string: Color32,
    /// Colour for [`TextToken::Number`].
    pub number: Color32,
    /// Colour for [`TextToken::Boolean`].
    pub boolean: Color32,
    /// Colour for [`TextToken::Bracket`].
    pub bracket: Color32,
}

impl TextPalette {
    /// Build the palette from the host-injected [`ThemeColors`] in egui memory.
    pub fn from_ctx(ctx: &egui::Context) -> Self {
        let tc = ThemeColors::from_ctx(ctx);
        Self {
            key: tc.syntax_key,
            string: tc.syntax_string,
            number: tc.syntax_number,
            boolean: tc.syntax_bool,
            bracket: tc.syntax_punctuation,
        }
    }

    /// The colour for `token`.
    pub fn color(&self, token: TextToken) -> Color32 {
        match token {
            TextToken::Key => self.key,
            TextToken::Str => self.string,
            TextToken::Number => self.number,
            TextToken::Boolean => self.boolean,
            TextToken::Bracket => self.bracket,
        }
    }

    /// The token colour when `enabled`, otherwise `base`.
    pub fn color_with_highlighting(&self, token: TextToken, enabled: bool, base: Color32) -> Color32 {
        if enabled { self.color(token) } else { base }
    }
}

/// Returns a [`egui::FontId`] that resolves to the Phosphor icon font family.
///
/// The host is expected to register the icon font under the
/// `FontFamily::Name("phosphor")` family (it does so in its font setup); SDK
/// widgets use this helper to render inline icons without depending on the
/// icon-font crate directly.
pub fn phosphor_font_id(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("phosphor".into()))
}
