#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

/// An animated on/off toggle switch.
///
/// Reports interaction through its [`egui::Widget`] response: a click means the
/// caller should flip `enabled` (standard immediate-mode flow — store the new
/// value and pass it back next frame).
///
/// ```
/// use thoth_plugin_sdk::components::ToggleSwitch;
///
/// let toggle = ToggleSwitch::builder().enabled(true).build();
/// ```
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Builder)]
pub struct ToggleSwitch<'a> {
    /// Whether the switch is currently on.
    #[builder(default)]
    #[serde(default)]
    pub enabled: bool,
    /// Optional tooltip shown on hover.
    #[serde(default)]
    pub hover_text: Option<&'a str>,
}
