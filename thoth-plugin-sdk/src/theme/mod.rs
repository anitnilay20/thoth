//! The library-owned theme palette and colour helpers.
//!
//! [`ThemeColors`] is the canonical palette type; the host publishes it into
//! egui memory (under [`THEME_MEMORY_ID`]) and SDK widgets read it back via
//! [`ThemeColors::from_ctx`]. Also exposes colour parsing/token resolution
//! ([`resolve_color`], [`parse_hex_color`], [`color_to_hex`]), contrast and
//! Phosphor-font helpers, and the [`TextPalette`]/[`TextToken`] syntax colours.

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
#[derive(Debug, Clone, Copy, Default)]
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

#[cfg(feature = "egui")]
impl ThemeColors {
    /// Build an `egui_code_editor` `ColorTheme` from the current palette so the
    /// code editor's syntax highlighting matches the active theme.
    ///
    /// Called per-frame; `hex()` interns each unique colour in a static cache to
    /// avoid per-frame allocations.
    pub fn code_editor_theme(&self) -> egui_code_editor::ColorTheme {
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
        egui_code_editor::ColorTheme {
            name: if is_dark { "Thoth Dark" } else { "Thoth Light" },
            dark: is_dark,
            // Match the surrounding panel background so the editor blends in
            // rather than reading as a distinct sunken box.
            bg: hex(self.bg),
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

/// Resolve a colour string to a [`Color32`], accepting either a `#rrggbb(aa)`
/// hex literal *or* a semantic theme token resolved against `colors`.
///
/// Tokens (theme-reactive): `fg`, `muted`/`fg-muted`/`gray`, `accent`/`blue`,
/// `secondary`/`purple`, `success`/`green`, `warning`/`orange`, `error`/`red`,
/// `info`/`cyan`, `string`, `number`, `bool`, `key`. Unknown strings fall back
/// to bare-hex parsing, then `None`.
pub fn resolve_color(token: &str, colors: &ThemeColors) -> Option<Color32> {
    if token.starts_with('#') {
        return parse_hex_color(token);
    }
    Some(match token {
        "fg" => colors.fg,
        "muted" | "fg-muted" | "gray" => colors.fg_muted,
        "accent" | "blue" => colors.accent,
        "secondary" | "purple" => colors.accent_secondary,
        "success" | "green" => colors.success,
        "warning" | "orange" => colors.warning,
        "error" | "red" => colors.error,
        "info" | "cyan" => colors.info,
        "string" => colors.syntax_string,
        "number" => colors.syntax_number,
        "bool" | "boolean" => colors.syntax_bool,
        "key" => colors.syntax_key,
        other => return parse_hex_color(other),
    })
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
    pub fn color_with_highlighting(
        &self,
        token: TextToken,
        enabled: bool,
        base: Color32,
    ) -> Color32 {
        if enabled { self.color(token) } else { base }
    }
}

/// Format a [`Color32`] as a `#rrggbbaa` hex string (inverse of
/// [`parse_hex_color`]).
pub fn color_to_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r(), c.g(), c.b(), c.a())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_color_6_digit_is_opaque() {
        assert_eq!(
            parse_hex_color("#ff8800"),
            Some(Color32::from_rgb(255, 136, 0))
        );
        assert_eq!(parse_hex_color("#ffffff"), Some(Color32::WHITE));
    }

    #[test]
    fn parse_hex_color_8_digit_carries_alpha() {
        // 8-digit hex is parsed as unmultiplied RGBA (egui then premultiplies).
        assert_eq!(
            parse_hex_color("#11223380"),
            Some(Color32::from_rgba_unmultiplied(0x11, 0x22, 0x33, 0x80)),
        );
        assert_eq!(parse_hex_color("#11223380").map(|c| c.a()), Some(0x80));
    }

    #[test]
    fn parse_hex_color_rejects_garbage() {
        assert_eq!(parse_hex_color("not-a-color"), None);
        assert_eq!(parse_hex_color("#xyz"), None);
        assert_eq!(parse_hex_color(""), None);
    }

    #[test]
    fn color_to_hex_round_trips_through_parse() {
        // Opaque colours round-trip exactly (premultiplied == unmultiplied at a=255).
        let original = Color32::from_rgb(0x12, 0x34, 0x56);
        let hex = color_to_hex(original);
        assert_eq!(hex, "#123456ff");
        assert_eq!(parse_hex_color(&hex), Some(original));
    }

    #[test]
    fn resolve_color_maps_semantic_tokens_to_palette() {
        let c = ThemeColors {
            syntax_string: Color32::from_rgb(1, 2, 3),
            syntax_number: Color32::from_rgb(4, 5, 6),
            syntax_bool: Color32::from_rgb(7, 8, 9),
            fg_muted: Color32::from_rgb(10, 11, 12),
            accent: Color32::from_rgb(13, 14, 15),
            ..Default::default()
        };

        assert_eq!(resolve_color("string", &c), Some(c.syntax_string));
        assert_eq!(resolve_color("number", &c), Some(c.syntax_number));
        assert_eq!(resolve_color("bool", &c), Some(c.syntax_bool));
        assert_eq!(resolve_color("boolean", &c), Some(c.syntax_bool));
        assert_eq!(resolve_color("muted", &c), Some(c.fg_muted));
        assert_eq!(resolve_color("blue", &c), Some(c.accent));
    }

    #[test]
    fn resolve_color_passes_through_hex_and_rejects_unknown() {
        let c = ThemeColors::default();
        assert_eq!(resolve_color("#010203", &c), parse_hex_color("#010203"));
        assert_eq!(resolve_color("definitely-not-a-token", &c), None);
    }

    #[test]
    fn contrast_text_color_is_white_on_dark_and_black_on_light() {
        assert_eq!(get_contrast_text_color(Color32::BLACK), Color32::WHITE);
        assert_eq!(get_contrast_text_color(Color32::WHITE), Color32::BLACK);
    }
}
