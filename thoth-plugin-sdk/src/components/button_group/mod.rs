#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::components::Button;

/// A segmented control: a row of [`Button`]s where one is the active segment.
///
/// Build it from a list of buttons and the index of the initially-active one.
/// When rendered (with the `egui` feature) use
/// [`ButtonGroups::show`](Self::show) to learn which segment the user picked;
/// the plain `egui::Widget` impl discards that selection.
///
/// ```
/// use thoth_plugin_sdk::components::{Button, ButtonGroups};
///
/// let group = ButtonGroups::builder()
///     .items(vec![
///         Button::builder().label("GET").build(),
///         Button::builder().label("POST").build(),
///     ])
///     .active(0)
///     .build();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct ButtonGroups {
    /// Widget id used for event routing.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// The segments, in display order.
    items: Vec<Button>,
    /// Index into `items` of the currently-active segment.
    active: usize,
}
