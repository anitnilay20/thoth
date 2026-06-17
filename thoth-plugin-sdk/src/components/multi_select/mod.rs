use bon::Builder;
use serde::{Deserialize, Serialize};

use super::SelectOption;

/// A checkbox list selecting multiple values. Owns the selected `value` set;
/// [`MultiSelect::show`] updates it in place.
///
/// ```
/// use thoth_plugin_sdk::components::{MultiSelect, SelectOption};
///
/// let mut m = MultiSelect::builder()
///     .options(vec![SelectOption::builder().value("a").label("A").build()])
///     .build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct MultiSelect {
    /// Optional group label shown above the options.
    #[builder(default)]
    #[serde(default)]
    pub label: String,
    /// Currently selected option values.
    #[builder(default)]
    #[serde(default)]
    pub value: Vec<String>,
    /// Available options.
    #[builder(default)]
    #[serde(default)]
    pub options: Vec<SelectOption>,
    /// Disable interaction.
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
}

#[cfg(feature = "egui")]
impl MultiSelect {
    /// Render the list, updating [`value`](MultiSelect::value) in place.
    /// Returns `true` if the selection changed this frame.
    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        ui.add_enabled_ui(!self.disabled, |ui| {
            ui.vertical(|ui| {
                if !self.label.is_empty() {
                    ui.label(&self.label);
                }
                for opt in &self.options {
                    let mut on = self.value.contains(&opt.value);
                    if ui.checkbox(&mut on, &opt.label).changed() {
                        changed = true;
                        if on {
                            self.value.push(opt.value.clone());
                        } else {
                            self.value.retain(|v| v != &opt.value);
                        }
                    }
                }
            });
        });
        changed
    }
}
