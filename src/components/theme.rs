use eframe::egui::{self, Color32};
use serde_json::Value;

pub fn apply_theme(ctx: &egui::Context, dark: bool) {
    if dark {
        ctx.set_visuals(andromeda_visuals());
        // small spacing/rounding polish
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        // style.visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);
        // style.visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);
        // style.visuals.widgets.active.rounding = egui::Rounding::same(6.0);
        ctx.set_style(style);
    } else {
        ctx.set_visuals(egui::Visuals::light());
    }
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
    /// Andromeda-flavored picks for a dark UI.
    pub const fn dark() -> Self {
        Self {
            // Purples / greens / oranges / blues common in Andromeda-style syntax themes
            key: Color32::from_rgb(179, 157, 219),
            string: Color32::from_rgb(195, 232, 141),
            number: Color32::from_rgb(247, 140, 108),
            boolean: Color32::from_rgb(130, 170, 255),
            bracket: Color32::from_rgb(137, 221, 255),
        }
    }

    /// Tuned for good contrast on light backgrounds.
    pub const fn light() -> Self {
        Self {
            key: Color32::from_rgb(57, 73, 171),
            string: Color32::from_rgb(34, 139, 34),
            number: Color32::from_rgb(196, 85, 0),
            boolean: Color32::from_rgb(21, 101, 192),
            bracket: Color32::from_rgb(84, 110, 122),
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
        Color32::from_rgb(0x3a, 0x3f, 0x49) // dark mode selection color
    } else {
        Color32::from_rgb(0xB0, 0xC4, 0xE0) // light mode selection color
    }
}
