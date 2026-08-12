#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use super::SelectOption;

/// A horizontal radio-button group. Owns the selected `value`; [`Radio::show`]
/// updates it in place and returns the newly-selected value when it changes.
///
/// ```
/// use thoth_plugin_sdk::components::{Radio, SelectOption};
///
/// let mut r = Radio::builder()
///     .value("a")
///     .options(vec![
///         SelectOption::builder().value("a").label("A").build(),
///         SelectOption::builder().value("b").label("B").build(),
///     ])
///     .build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Radio {
    /// Widget id used for event routing.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// Optional group label shown above the options.
    #[builder(default)]
    #[serde(default)]
    pub label: String,
    /// Currently selected option value.
    #[builder(default)]
    #[serde(default)]
    pub value: String,
    /// Available options.
    #[builder(default)]
    #[serde(default)]
    pub options: Vec<SelectOption>,
    /// Disable interaction.
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
}
