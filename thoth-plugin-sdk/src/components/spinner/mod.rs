use bon::Builder;
use serde::{Deserialize, Serialize};

/// An indeterminate loading spinner.
///
/// ```
/// use thoth_plugin_sdk::components::Spinner;
///
/// let spinner = Spinner::builder().size(20.0).build();
/// ```
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Builder)]
#[non_exhaustive]
pub struct Spinner {
    /// Diameter in points; defaults to 16.
    #[serde(default)]
    pub size: Option<f32>,
}

#[cfg(feature = "egui")]
impl egui::Widget for Spinner {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        use crate::theme::ThemeColors;
        let colors = ThemeColors::from_ctx(ui.ctx());
        let spinner = egui::Spinner::new()
            .color(colors.accent)
            .size(self.size.unwrap_or(16.0));
        ui.add(spinner)
    }
}
