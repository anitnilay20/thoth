#[cfg(feature = "egui")]
pub mod ui;

use crate::helpers::default_enabled;
use bon::Builder;
use serde::{Deserialize, Serialize};

/// Visual style of a [`Button`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonType {
    /// Filled button with a solid background — the default, for primary actions.
    #[default]
    Elevated,
    /// Borderless text-only button — for low-emphasis / inline actions.
    Text,
}

/// Preset size of a [`Button`], controlling font size and height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum ButtonSize {
    /// Compact: 11pt text, 24px tall.
    Small,
    /// Default: 13pt text, 28px tall.
    #[default]
    Medium,
    /// Prominent: 15pt text, 32px tall.
    Large,
}

impl ButtonSize {
    /// Returns this size's `(font_size, height)` in points/pixels.
    pub fn metrics(self) -> (f32, f32) {
        match self {
            ButtonSize::Small => (11.0, 24.0),
            ButtonSize::Medium => (13.0, 28.0),
            ButtonSize::Large => (15.0, 32.0),
        }
    }
}

/// Semantic colour role of a [`Button`].
///
/// The role is resolved against the active theme at render time (see
/// [`crate::theme::ThemeColors`]); buttons never carry raw colours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum ButtonColor {
    /// Neutral surface fill — the default.
    #[default]
    Default,
    /// Primary accent — the main call to action.
    Primary,
    /// Secondary accent — complementary emphasis.
    Secondary,
    /// Destructive action (delete, discard).
    Danger,
    /// Positive / confirming action.
    Success,
}

/// A clickable button.
///
/// Construct one with the [`bon`] builder via [`Button::builder`]; only
/// [`label`](Button::label) is required and every other field has a sensible
/// default.
///
/// ```
/// use thoth_plugin_sdk::components::{Button, ButtonColor};
///
/// let button = Button::builder()
///     .label("Save")
///     .color(ButtonColor::Primary)
///     .build();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Button {
    /// Widget id used for event routing (empty if not part of a DSL tree).
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// Text shown on the button.
    pub label: String,
    /// Visual style (elevated vs. text). Defaults to [`ButtonType::Elevated`].
    #[builder(default)]
    #[serde(rename = "button-type", default)]
    pub button_type: ButtonType,
    /// Semantic colour role. Defaults to [`ButtonColor::Default`].
    #[builder(default)]
    #[serde(default)]
    pub color: ButtonColor,
    /// Preset size. Defaults to [`ButtonSize::Medium`].
    #[builder(default)]
    #[serde(default)]
    pub button_size: ButtonSize,
    /// Optional tooltip shown on hover.
    #[serde(default)]
    pub hover_text: Option<String>,
    /// Font size override — if unset, derived from `button_size`.
    #[serde(default)]
    pub size: Option<f32>,
    /// Fixed width override in pixels — if unset, sized to fit the label.
    #[serde(default)]
    pub width: Option<f32>,
    /// Height override — if unset, derived from `button_size`.
    #[serde(default)]
    pub height: Option<f32>,
    /// Whether the button is interactive. Defaults to `true`; a disabled
    /// button is dimmed and ignores clicks.
    #[builder(default = true)]
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Optional leading icon — a Phosphor glyph rendered before the label.
    #[serde(default)]
    pub icon: Option<String>,
    /// When set, clicking copies this text to the clipboard (handled in-widget,
    /// no plugin round-trip).
    #[serde(default)]
    pub copy: Option<String>,
    /// Stretch the button to the full available width of its container.
    #[builder(default)]
    #[serde(rename = "full-width", default)]
    pub full_width: bool,
}
