#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use super::SelectOption;
use crate::components::Size;

/// A dropdown selecting multiple values: a trigger summarising the selection
/// count, opening a popover of checkbox rows. Owns the selected `value` set;
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
#[non_exhaustive]
pub struct MultiSelect {
    /// Widget id used for event routing.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// Optional group label shown above the trigger.
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
    /// Singular noun used in the trigger's count summary, e.g. `"column"` renders
    /// as `"2 columns"`. When `None` the summary reads `"2 selected"`.
    #[serde(default)]
    pub item_noun: Option<String>,
    /// Trigger size. Defaults to [`Size::Medium`].
    #[builder(default)]
    #[serde(default)]
    pub size: Size,
    /// Fixed trigger width. When `None`, the trigger fills the available width.
    #[serde(default)]
    pub width: Option<f32>,
}
