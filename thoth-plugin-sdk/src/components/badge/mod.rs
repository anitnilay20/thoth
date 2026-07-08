use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::components::Size;

/// A small colored pill label (e.g. an HTTP method or status tag).
///
/// ```
/// use thoth_plugin_sdk::components::Badge;
///
/// let badge = Badge::builder().label("GET").color("#89b4fa").build();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Badge {
    /// Text shown inside the pill.
    pub label: String,
    /// Fill colour as a `#rrggbb` hex string; defaults to the secondary accent.
    #[serde(default)]
    pub color: Option<String>,
    /// When true, render as an outlined pill (transparent fill, coloured 1px
    /// border and coloured monospace text) instead of a filled one.
    #[builder(default)]
    #[serde(default)]
    pub outlined: bool,
    /// When true, render as a soft pill: a faint tint of `color` filled behind
    /// coloured monospace text (the enum-value chip style). Takes precedence
    /// over [`outlined`](Badge::outlined).
    #[builder(default)]
    #[serde(default)]
    pub soft: bool,
    /// Pill size (font + padding). Defaults to [`Size::Small`] — a slim pill.
    #[builder(default = Size::Small)]
    #[serde(default = "default_badge_size")]
    pub size: Size,
}

fn default_badge_size() -> Size {
    Size::Small
}

#[cfg(feature = "egui")]
impl egui::Widget for Badge {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        use crate::theme::{ThemeColors, get_contrast_text_color, resolve_color};
        let colors = ThemeColors::from_ctx(ui.ctx());
        let color = self
            .color
            .as_deref()
            .and_then(|c| resolve_color(c, &colors))
            .unwrap_or(colors.accent_secondary);

        // Slim by default; padding + font scale with the size. Vertical padding
        // is 0 at the small size so the pill hugs the text (matches the handoff's
        // 1px-tall enum chip).
        let (font, pad_x, pad_y): (f32, i8, i8) = match self.size {
            Size::Small => (9.0, 6, 0),
            Size::Medium => (10.0, 7, 1),
            Size::Large => (12.0, 9, 2),
        };
        let margin = egui::Margin::symmetric(pad_x, pad_y);

        if self.soft {
            // A faint tint of the colour behind coloured text (the chip look) —
            // ~0.18 alpha, matching the handoff.
            let bg = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 46);
            egui::Frame::new()
                .fill(bg)
                .corner_radius(8.0)
                .inner_margin(margin)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(&self.label)
                            .monospace()
                            .size(font)
                            .color(color),
                    );
                })
                .response
        } else if self.outlined {
            // Transparent fill, coloured border + coloured monospace text — the
            // schema/structure constraint-tag style.
            egui::Frame::new()
                .stroke(egui::Stroke::new(1.0, color))
                .corner_radius(3.0)
                .inner_margin(margin)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(&self.label)
                            .monospace()
                            .size(font)
                            .color(color),
                    );
                })
                .response
        } else {
            let fg = get_contrast_text_color(color);
            egui::Frame::new()
                .fill(color)
                .corner_radius(3.0)
                .inner_margin(margin)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(&self.label).size(font).color(fg));
                })
                .response
        }
    }
}
