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
pub struct Badge {
    /// Text shown inside the pill.
    pub label: String,
    /// Fill colour as a `#rrggbb` hex string; defaults to the secondary accent.
    #[serde(default)]
    pub color: Option<String>,
}

#[cfg(feature = "egui")]
impl egui::Widget for Badge {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        use crate::theme::{ThemeColors, resolve_color};
        let colors = ThemeColors::from_ctx(ui.ctx());
        let bg = self
            .color
            .as_deref()
            .and_then(|c| resolve_color(c, &colors))
            .unwrap_or(colors.accent_secondary);
        egui::Frame::new()
            .fill(bg)
            .corner_radius(3.0)
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, |ui| {
                ui.label(&self.label);
            })
            .response
    }
}
