#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

fn default_rows() -> usize {
    4
}

/// A single- or multi-line text input.
///
/// Unlike the stateless display widgets, an `Input` owns its editable
/// [`value`](Input::value). Render it with
/// [`show`](Input::show), which mutates `value` in place and reports whether it
/// changed this frame — hold the `Input` in your own state across frames.
///
/// ```
/// use thoth_plugin_sdk::components::Input;
///
/// let mut field = Input::builder().placeholder("Search…").build();
/// // each frame: field.show(ui); then read field.value
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
pub struct Input {
    /// Current text content (mutated in place by [`Input::show`]).
    #[builder(default)]
    #[serde(default)]
    pub value: String,
    /// Ghost text shown when the field is empty.
    #[builder(default)]
    #[serde(default)]
    pub placeholder: String,
    /// Optional leading Phosphor icon glyph (single-line only).
    #[serde(default)]
    pub icon: Option<String>,
    /// Mask the text as bullets (password field).
    #[builder(default)]
    #[serde(default)]
    pub password: bool,
    /// Disable interaction.
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
    /// Render as a multi-line text area.
    #[builder(default)]
    #[serde(default)]
    pub multiline: bool,
    /// Visible row count when `multiline` is true. Defaults to 4.
    #[builder(default = default_rows())]
    #[serde(default = "default_rows")]
    pub rows: usize,
    /// `None` fills the available width; `Some(w)` fixes the width to `w`.
    #[serde(default)]
    pub desired_width: Option<f32>,
}
