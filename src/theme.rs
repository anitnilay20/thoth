use eframe::egui::{self, Color32};
use serde_json::Value;

use crate::settings::Settings;

/// Apply theme settings including visuals and fonts
pub fn apply_theme(ctx: &egui::Context, settings: &Settings) {
    // Apply visual theme (dark/light mode)
    if settings.dark_mode {
        ctx.set_visuals(andromeda_visuals());
    } else {
        ctx.set_visuals(egui::Visuals::light());
    }

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
    style.spacing.item_spacing = egui::vec2(8.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.indent = 16.0; // Match our tree indent

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

    ctx.set_style(style);
}

/// Catppuccin Mocha theme colors
pub mod catppuccin_mocha {
    use eframe::egui::Color32 as C;

    // Base colors
    pub const ROSEWATER: C = C::from_rgb(0xf5, 0xe0, 0xdc);
    pub const FLAMINGO: C = C::from_rgb(0xf2, 0xcd, 0xcd);
    pub const PINK: C = C::from_rgb(0xf5, 0xc2, 0xe7);
    pub const MAUVE: C = C::from_rgb(0xcb, 0xa6, 0xf7);
    pub const RED: C = C::from_rgb(0xf3, 0x8b, 0xa8);
    pub const MAROON: C = C::from_rgb(0xeb, 0xa0, 0xac);
    pub const PEACH: C = C::from_rgb(0xfa, 0xb3, 0x87);
    pub const YELLOW: C = C::from_rgb(0xf9, 0xe2, 0xaf);
    pub const GREEN: C = C::from_rgb(0xa6, 0xe3, 0xa1);
    pub const TEAL: C = C::from_rgb(0x94, 0xe2, 0xd5);
    pub const SKY: C = C::from_rgb(0x89, 0xdc, 0xeb);
    pub const SAPPHIRE: C = C::from_rgb(0x74, 0xc7, 0xec);
    pub const BLUE: C = C::from_rgb(0x89, 0xb4, 0xfa);
    pub const LAVENDER: C = C::from_rgb(0xb4, 0xbe, 0xfe);

    // Text colors
    pub const TEXT: C = C::from_rgb(0xcd, 0xd6, 0xf4);
    pub const SUBTEXT1: C = C::from_rgb(0xba, 0xc2, 0xde);
    pub const SUBTEXT0: C = C::from_rgb(0xa6, 0xad, 0xc8);

    // Overlay colors
    pub const OVERLAY2: C = C::from_rgb(0x93, 0x99, 0xb2);
    pub const OVERLAY1: C = C::from_rgb(0x7f, 0x84, 0x9c);
    pub const OVERLAY0: C = C::from_rgb(0x6c, 0x70, 0x86);

    // Surface colors
    pub const SURFACE2: C = C::from_rgb(0x58, 0x5b, 0x70);
    pub const SURFACE1: C = C::from_rgb(0x45, 0x47, 0x5a);
    pub const SURFACE0: C = C::from_rgb(0x31, 0x32, 0x44);

    // Base backgrounds
    pub const BASE: C = C::from_rgb(0x1e, 0x1e, 0x2e);
    pub const MANTLE: C = C::from_rgb(0x18, 0x18, 0x25);
    pub const CRUST: C = C::from_rgb(0x11, 0x11, 0x1b);
}

fn andromeda_visuals() -> egui::Visuals {
    use catppuccin_mocha as ctp;
    let mut v = egui::Visuals::dark();

    // Catppuccin Mocha palette
    v.override_text_color = Some(ctp::TEXT);
    v.panel_fill = ctp::BASE; // Main background
    v.extreme_bg_color = ctp::MANTLE; // Panels/cards
    v.faint_bg_color = ctp::SURFACE0; // Alt rows

    // Widget colors
    v.widgets.noninteractive.bg_fill = ctp::SURFACE0;
    v.widgets.inactive.bg_fill = ctp::SURFACE0;
    v.widgets.hovered.bg_fill = ctp::SURFACE1;
    v.widgets.active.bg_fill = ctp::SURFACE2;

    // Selection and accent colors
    v.selection.bg_fill = ctp::MAUVE;
    v.selection.stroke = egui::Stroke::new(1.0, ctp::MAUVE);
    v.hyperlink_color = ctp::BLUE;

    v
}

pub fn row_fill(i: usize, ui: &egui::Ui) -> Color32 {
    if i % 2 == 1 {
        // Only paint odd rows
        if ui.visuals().dark_mode {
            catppuccin_mocha::SURFACE0 // Catppuccin alternating row
        } else {
            Color32::from_rgb(0xEC, 0xEE, 0xF3) // Light stripe for light mode
        }
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

impl From<&mut Value> for TextToken {
    fn from(value: &mut Value) -> Self {
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
    /// Catppuccin Mocha dark theme colors
    pub const fn dark() -> Self {
        use catppuccin_mocha as ctp;
        Self {
            key: ctp::BLUE,         // Keys/Properties - Blue
            string: ctp::GREEN,     // String values - Green
            number: ctp::PEACH,     // Numbers - Peach
            boolean: ctp::MAUVE,    // Booleans - Mauve
            bracket: ctp::OVERLAY2, // Brackets/Punctuation - Overlay2
        }
    }

    /// Light theme colors
    pub const fn light() -> Self {
        Self {
            key: Color32::from_rgb(0, 16, 128),     // Keys (dark blue)
            string: Color32::from_rgb(163, 21, 21), // Strings (red)
            number: Color32::from_rgb(9, 134, 88),  // Numbers (green)
            boolean: Color32::from_rgb(0, 0, 255),  // Booleans (blue)
            bracket: Color32::from_rgb(0, 0, 0),    // Brackets (black)
        }
    }

    /// Convenience: choose light/dark automatically from egui's visuals.
    pub fn for_visuals(visuals: &egui::Visuals) -> Self {
        if visuals.dark_mode {
            Self::dark()
        } else {
            Self::light()
        }
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
}

pub fn selected_row_bg(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        // Catppuccin Mauve with reduced opacity for selection
        Color32::from_rgba_premultiplied(203, 166, 247, 77)
    } else {
        // Light mode: similar selection color
        Color32::from_rgba_premultiplied(14, 99, 156, 102)
    }
}

/// Hover overlay for rows
pub fn hover_row_bg(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        // Catppuccin Surface1 for subtle hover
        catppuccin_mocha::SURFACE1
    } else {
        // Light mode: subtle dark overlay
        Color32::from_rgba_premultiplied(0, 0, 0, 13)
    }
}
