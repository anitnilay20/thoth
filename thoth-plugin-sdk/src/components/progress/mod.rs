use bon::Builder;
use serde::{Deserialize, Serialize};

/// A horizontal progress bar.
///
/// ```
/// use thoth_plugin_sdk::components::Progress;
///
/// let bar = Progress::builder().value(0.6).build();
/// ```
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Builder)]
#[non_exhaustive]
pub struct Progress {
    /// Completion in `0.0..=1.0`.
    pub value: f64,
}

#[cfg(feature = "egui")]
impl egui::Widget for Progress {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.add(egui::ProgressBar::new(self.value.clamp(0.0, 1.0) as f32))
    }
}
