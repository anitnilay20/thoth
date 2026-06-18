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
    /// Widget id used for event routing.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// Label shown above the field.
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
        if !self.label.is_empty() {
            ui.label(&self.label);
        }
        let range = self.min.unwrap_or(f64::NEG_INFINITY)..=self.max.unwrap_or(f64::INFINITY);
        let drag = egui::DragValue::new(&mut self.value).range(range);
        ui.add_enabled(!self.disabled, drag)
    }
}
