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

fn andromeda_visuals() -> egui::Visuals {
    use egui::Color32 as C;
    let mut v = egui::Visuals::dark();

    // Andromeda-ish palette
    let bg0 = C::from_rgb(0x1f, 0x22, 0x30); // window bg
    let bg1 = C::from_rgb(0x23, 0x26, 0x2e); // panels / cards
    let bg2 = C::from_rgb(0x1b, 0x1e, 0x28); // alt rows
    let txt = C::from_rgb(0xe6, 0xe6, 0xe6);
    let acc = C::from_rgb(0x82, 0xaa, 0xff);

    v.override_text_color = Some(txt);
    v.panel_fill = bg0;
    v.extreme_bg_color = bg1;
    v.widgets.noninteractive.bg_fill = bg1;
    v.widgets.inactive.bg_fill = bg1;
    v.widgets.hovered.bg_fill = C::from_rgb(0x2a, 0x2f, 0x3c);
    v.widgets.active.bg_fill = C::from_rgb(0x34, 0x3a, 0x49);
    v.faint_bg_color = bg2;

    v.selection.bg_fill = acc;
    v.selection.stroke = egui::Stroke::new(1.0, acc);
    v.hyperlink_color = acc;
    // v.window_rounding = egui::Rounding::same(10.0);
    v
}

pub fn row_fill(i: usize, ui: &egui::Ui) -> Color32 {
    if i % 2 == 1 {
        // Only paint odd rows
        if ui.visuals().dark_mode {
            Color32::from_rgb(0x25, 0x28, 0x33) // lighter stripe
        } else {
            Color32::from_rgb(0xEC, 0xEE, 0xF3) // light stripe for light mode
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
    /// VS Code-inspired dark theme colors
    pub const fn dark() -> Self {
        Self {
            // VS Code syntax highlighting colors
            key: Color32::from_rgb(156, 220, 254), // #9cdcfe - Keys/Properties
            string: Color32::from_rgb(206, 145, 120), // #ce9178 - String values
            number: Color32::from_rgb(181, 206, 168), // #b5cea8 - Numbers
            boolean: Color32::from_rgb(86, 156, 214), // #569cd6 - Booleans
            bracket: Color32::from_rgb(212, 212, 212), // #d4d4d4 - Brackets/Punctuation
        }
    }

    /// VS Code-inspired light theme colors
    pub const fn light() -> Self {
        Self {
            key: Color32::from_rgb(0, 16, 128), // #001080 - Keys (dark blue)
            string: Color32::from_rgb(163, 21, 21), // #a31515 - Strings (red)
            number: Color32::from_rgb(9, 134, 88), // #098658 - Numbers (green)
            boolean: Color32::from_rgb(0, 0, 255), // #0000ff - Booleans (blue)
            bracket: Color32::from_rgb(0, 0, 0), // #000000 - Brackets (black)
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
        // VS Code selection color: #0e639c with 30% opacity (4d = 77/255)
        Color32::from_rgba_premultiplied(14, 99, 156, 77)
    } else {
        // Light mode: similar blue selection with adjusted opacity
        Color32::from_rgba_premultiplied(14, 99, 156, 102)
    }
}

/// Hover overlay for rows (5% white overlay in dark mode)
pub fn hover_row_bg(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        // VS Code hover: #ffffff with 5% opacity (0d = 13/255)
        Color32::from_rgba_premultiplied(255, 255, 255, 13)
    } else {
        // Light mode: subtle dark overlay
        Color32::from_rgba_premultiplied(0, 0, 0, 13)
    }
}
