#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

/// A single option in a [`Select`].
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct SelectOption {
    /// Stable value matched against [`Select::value`].
    pub value: String,
    /// Human-readable label shown in the list.
    pub label: String,
}

/// Trigger size of a [`Select`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SelectSize {
    /// 28px trigger, 11pt text — the default (matches [`ButtonSize::Medium`]).
    ///
    /// [`ButtonSize::Medium`]: crate::components::ButtonSize::Medium
    #[default]
    Default,
    /// 24px trigger, 10pt text (matches [`ButtonSize::Small`]).
    ///
    /// [`ButtonSize::Small`]: crate::components::ButtonSize::Small
    Small,
}

/// A dropdown select (combo box) with a custom-painted trigger and popup list.
///
/// Stateful: it owns the currently-selected [`value`](Select::value). Render
/// with [`show`](Select::show), which updates `value` on selection and reports
/// the newly-chosen value.
///
/// ```
/// use thoth_plugin_sdk::components::{Select, SelectOption};
///
/// let select = Select::builder()
///     .id("sort")
///     .value("name")
///     .options(vec![
///         SelectOption::builder().value("name").label("Name").build(),
///         SelectOption::builder().value("date").label("Date").build(),
///     ])
///     .build();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Select {
    /// Stable id salt — must be unique per on-screen instance (used for the
    /// open/closed popup state and event routing).
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// Currently selected value (matched against [`SelectOption::value`]).
    #[builder(default)]
    #[serde(default)]
    pub value: String,
    /// Available options, in display order.
    #[builder(default)]
    #[serde(default)]
    pub options: Vec<SelectOption>,
    /// Optional static prefix shown before the selected label, e.g. `"Sort: "`.
    #[serde(default)]
    pub prefix_label: Option<String>,
    /// Trigger size. Defaults to [`SelectSize::Default`].
    #[builder(default)]
    #[serde(default)]
    pub size: SelectSize,
    /// Fixed trigger width. When `None`, the trigger fills the available width.
    #[serde(default)]
    pub width: Option<f32>,
}
