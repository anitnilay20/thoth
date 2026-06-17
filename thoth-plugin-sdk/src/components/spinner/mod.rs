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
pub struct Spinner {
    /// Diameter in points; defaults to egui's default spinner size.
    #[serde(default)]
    pub size: Option<f32>,
}

#[cfg(feature = "egui")]
impl egui::Widget for Spinner {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        use crate::theme::ThemeColors;
        let colors = ThemeColors::from_ctx(ui.ctx());
        let mut spinner = egui::Spinner::new().color(colors.accent);
        if let Some(size) = self.size {
            spinner = spinner.size(size);
        }
        ui.add(spinner)
    }
}
