use bon::Builder;
use serde::{Deserialize, Serialize};

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
        if self.outlined {
            // Transparent fill, coloured border + coloured monospace text — the
            // schema/structure constraint-tag style.
            egui::Frame::new()
                .stroke(egui::Stroke::new(1.0, color))
                .corner_radius(3.0)
                .inner_margin(egui::Margin::symmetric(6, 2))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(&self.label)
                            .monospace()
                            .size(9.0)
                            .color(color),
                    );
                })
                .response
        } else {
            let fg = get_contrast_text_color(color);
            egui::Frame::new()
                .fill(color)
                .corner_radius(3.0)
                .inner_margin(egui::Margin::symmetric(4, 2))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(&self.label).color(fg));
                })
                .response
        }
    }
}
