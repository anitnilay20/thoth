use bon::Builder;
use serde::{Deserialize, Serialize};

/// A numeric drag/spin input with optional bounds. Owns its `value`;
/// [`NumberInput::show`] edits it in place.
///
/// ```
/// use thoth_plugin_sdk::components::NumberInput;
///
/// let mut n = NumberInput::builder().label("Port").value(8080.0).build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct NumberInput {
    /// Label shown before the field.
    #[builder(default)]
    #[serde(default)]
    pub label: String,
    /// Current value.
    pub value: f64,
    /// Optional minimum.
    #[serde(default)]
    pub min: Option<f64>,
    /// Optional maximum.
    #[serde(default)]
    pub max: Option<f64>,
    /// Disable interaction.
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
}

#[cfg(feature = "egui")]
impl NumberInput {
    /// Render the input, editing [`value`](NumberInput::value) in place.
    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal(|ui| {
            if !self.label.is_empty() {
                ui.label(&self.label);
            }
            let mut drag = egui::DragValue::new(&mut self.value);
            match (self.min, self.max) {
                (Some(min), Some(max)) => drag = drag.range(min..=max),
                (Some(min), None) => drag = drag.range(min..=f64::INFINITY),
                (None, Some(max)) => drag = drag.range(f64::NEG_INFINITY..=max),
                (None, None) => {}
            }
            ui.add_enabled(!self.disabled, drag)
        })
        .inner
    }
}
