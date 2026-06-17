use bon::Builder;
use serde::{Deserialize, Serialize};

/// A labelled slider over a numeric range. Owns its `value`; [`Slider::show`]
/// edits it in place.
///
/// ```
/// use thoth_plugin_sdk::components::Slider;
///
/// let mut s = Slider::builder().label("Opacity").value(0.5).min(0.0).max(1.0).build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct Slider {
    /// Label shown before the slider.
    #[builder(default)]
    #[serde(default)]
    pub label: String,
    /// Current value.
    pub value: f64,
    /// Range minimum.
    pub min: f64,
    /// Range maximum.
    pub max: f64,
    /// Disable interaction.
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
}

#[cfg(feature = "egui")]
impl Slider {
    /// Render the slider, editing [`value`](Slider::value) in place.
    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal(|ui| {
            if !self.label.is_empty() {
                ui.label(&self.label);
            }
            ui.add_enabled(
                !self.disabled,
                egui::Slider::new(&mut self.value, self.min..=self.max),
            )
        })
        .inner
    }
}
