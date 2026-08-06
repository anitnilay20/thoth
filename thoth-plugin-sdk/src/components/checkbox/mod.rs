#[cfg(feature = "egui")]
mod ui;

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
#[non_exhaustive]
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
    /// Mixed state — shows a minus glyph instead of a check, and takes
    /// precedence over [`checked`](Checkbox::checked) while set.
    #[builder(default)]
    #[serde(default)]
    pub indeterminate: bool,
    /// Disable interaction.
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
}
