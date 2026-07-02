use bon::Builder;
use serde::{Deserialize, Serialize};

/// A horizontal progress bar.
///
/// ```
/// use thoth_plugin_sdk::components::Progress;
///
/// let bar = Progress::builder().value(0.6).color("success").build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Progress {
    /// Completion in `0.0..=1.0`.
    pub value: f64,
    /// Optional fill colour — a semantic token (e.g. `"success"`, `"warning"`,
    /// `"info"`) or a `#rrggbb` hex. Defaults to the theme accent when unset.
    #[serde(default)]
    pub color: Option<String>,
    /// Optional fixed bar height in points.
    #[serde(default)]
    pub height: Option<f32>,
}

#[cfg(feature = "egui")]
impl egui::Widget for Progress {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut bar = egui::ProgressBar::new(self.value.clamp(0.0, 1.0) as f32);
        if let Some(token) = &self.color {
            let colors = crate::theme::ThemeColors::from_ctx(ui.ctx());
            if let Some(c) = crate::theme::resolve_color(token, &colors) {
                bar = bar.fill(c);
            }
        }
        if let Some(h) = self.height {
            bar = bar.desired_height(h);
        }
        ui.add(bar)
    }
}
