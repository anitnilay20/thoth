#[cfg(feature = "egui")]
mod ui;

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
#[non_exhaustive]
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
    /// Amount the `−` / `+` spin buttons add or subtract. Defaults to `1`; a
    /// value that isn't finite and positive (zero, negative, NaN, infinite) is
    /// normalised to `1` rather than making the buttons inert or reversed.
    #[serde(default)]
    pub step: Option<f64>,
    /// Optional unit suffix shown inside the control, e.g. `"rows"` — design
    /// `.num .unit`.
    #[serde(default)]
    pub unit: Option<String>,
    /// Disable interaction.
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
}
