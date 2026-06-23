use bon::Builder;
use serde::{Deserialize, Serialize};

/// A standalone Phosphor icon glyph.
///
/// ```
/// use thoth_plugin_sdk::components::Icon;
///
/// let icon = Icon::builder().glyph("\u{e3d0}").size(16.0).build();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Icon {
    /// The Phosphor glyph to render.
    pub glyph: String,
    /// Colour as a `#rrggbb` hex string; defaults to muted foreground.
    #[serde(default)]
    pub color: Option<String>,
    /// Glyph size in points; defaults to 13.
    #[serde(default)]
    pub size: Option<f32>,
}

#[cfg(feature = "egui")]
impl egui::Widget for Icon {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        use crate::theme::{ThemeColors, phosphor_font_id, resolve_color};
        let colors = ThemeColors::from_ctx(ui.ctx());
        let color = self
            .color
            .as_deref()
            .and_then(|c| resolve_color(c, &colors))
            .unwrap_or(colors.fg_muted);
        let size = self.size.unwrap_or(13.0);
        ui.label(egui::RichText::new(&self.glyph).font(phosphor_font_id(size)).color(color))
    }
}
