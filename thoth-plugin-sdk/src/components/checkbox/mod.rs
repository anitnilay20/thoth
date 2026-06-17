use bon::Builder;
use serde::{Deserialize, Serialize};

/// A labelled checkbox. Owns its `checked` state; [`Checkbox::show`] toggles it
/// in place and returns the widget response.
///
/// ```
/// use thoth_plugin_sdk::components::Checkbox;
///
/// let mut cb = Checkbox::builder().label("Enabled").checked(true).build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct Checkbox {
    /// Widget id used for event routing.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// Label shown next to the box.
    pub label: String,
    /// Whether the box is checked.
    #[builder(default)]
    #[serde(default)]
    pub checked: bool,
    /// Disable interaction.
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
}

#[cfg(feature = "egui")]
impl Checkbox {
    /// Render the checkbox, toggling [`checked`](Checkbox::checked) in place.
    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        ui.add_enabled(
            !self.disabled,
            egui::Checkbox::new(&mut self.checked, &self.label),
        )
    }
}
