#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

/// One segment of a [`ButtonGroups`] control.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct ButtonGroupItem {
    /// The value emitted when this segment is selected.
    pub value: String,
    /// The label shown on the segment.
    pub label: String,
}

/// A pill-style segmented control — one selection at a time.
///
/// The active segment is filled with `surface_active`; inactive segments are
/// transparent and highlight on hover. [`ButtonGroups::show`](Self::show) reports
/// the newly-selected value; the plain `egui::Widget` impl discards it.
///
/// ```
/// use thoth_plugin_sdk::components::{ButtonGroupItem, ButtonGroups};
///
/// let group = ButtonGroups::builder()
///     .items(vec![
///         ButtonGroupItem::builder().value("get").label("GET").build(),
///         ButtonGroupItem::builder().value("post").label("POST").build(),
///     ])
///     .active("get")
///     .build();
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct ButtonGroups {
    /// Widget id used for event routing.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// The segments, in display order.
    #[builder(default)]
    #[serde(default)]
    pub items: Vec<ButtonGroupItem>,
    /// The currently-active segment's value.
    #[builder(default)]
    #[serde(default)]
    pub active: String,
}
