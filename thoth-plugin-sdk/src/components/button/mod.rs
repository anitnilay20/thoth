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

#[cfg(test)]
mod tests {
    use super::{Button, ButtonColor, ButtonSize, ButtonType};
    use serde_json::Value;

    // ── builder defaults ──────────────────────────────────────────────────────

    #[test]
    fn builder_requires_only_label() {
        let btn = Button::builder().label("Save").build();
        assert_eq!(btn.label, "Save");
        assert_eq!(btn.id, "");
        assert_eq!(btn.button_type, ButtonType::Elevated);
        assert_eq!(btn.color, ButtonColor::Default);
        assert_eq!(btn.button_size, ButtonSize::Medium);
        assert!(btn.enabled);
        assert!(!btn.full_width);
        assert!(btn.icon.is_none());
    }

    #[test]
    fn builder_sets_all_fields() {
        let btn = Button::builder()
            .id("my-btn")
            .label("Delete")
            .color(ButtonColor::Danger)
            .button_type(ButtonType::Text)
            .button_size(ButtonSize::Small)
            .enabled(false)
            .full_width(true)
            .icon("trash-icon")
            .build();
        assert_eq!(btn.id, "my-btn");
        assert_eq!(btn.label, "Delete");
        assert_eq!(btn.color, ButtonColor::Danger);
        assert_eq!(btn.button_type, ButtonType::Text);
        assert_eq!(btn.button_size, ButtonSize::Small);
        assert!(!btn.enabled);
        assert!(btn.full_width);
        assert_eq!(btn.icon.as_deref(), Some("trash-icon"));
    }

    // ── ButtonSize::metrics ───────────────────────────────────────────────────

    #[test]
    fn button_size_small_metrics() {
        assert_eq!(ButtonSize::Small.metrics(), (11.0, 24.0));
    }

    #[test]
    fn button_size_medium_metrics() {
        assert_eq!(ButtonSize::Medium.metrics(), (13.0, 28.0));
    }

    #[test]
    fn button_size_large_metrics() {
        assert_eq!(ButtonSize::Large.metrics(), (15.0, 32.0));
    }

    // ── serialisation ─────────────────────────────────────────────────────────

    #[test]
    fn button_type_elevated_serialises() {
        let s = serde_json::to_string(&ButtonType::Elevated).unwrap();
        assert_eq!(s, r#""Elevated""#);
    }

    #[test]
    fn button_type_text_serialises() {
        let s = serde_json::to_string(&ButtonType::Text).unwrap();
        assert_eq!(s, r#""Text""#);
    }

    #[test]
    fn button_color_primary_serialises() {
        let s = serde_json::to_string(&ButtonColor::Primary).unwrap();
        assert_eq!(s, r#""Primary""#);
    }

    #[test]
    fn button_color_danger_serialises() {
        let s = serde_json::to_string(&ButtonColor::Danger).unwrap();
        assert_eq!(s, r#""Danger""#);
    }

    #[test]
    fn button_serialises_renamed_fields() {
        let btn = Button::builder()
            .label("Go")
            .button_type(ButtonType::Elevated)
            .full_width(true)
            .build();
        let v: Value = serde_json::to_value(&btn).unwrap();
        assert_eq!(v["button-type"], "Elevated");
        assert_eq!(v["full-width"], true);
    }

    #[test]
    fn button_round_trips_through_json() {
        let original = Button::builder()
            .id("btn")
            .label("Click")
            .color(ButtonColor::Success)
            .enabled(false)
            .build();
        let json = serde_json::to_string(&original).unwrap();
        let restored: Button = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "btn");
        assert_eq!(restored.label, "Click");
        assert_eq!(restored.color, ButtonColor::Success);
        assert!(!restored.enabled);
    }
}
