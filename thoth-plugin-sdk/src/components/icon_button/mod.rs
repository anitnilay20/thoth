#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::components::Size;

fn size_small() -> Size {
    Size::Small
}

/// A compact, square icon button rendered from a Phosphor glyph.
///
/// Reports clicks through its [`egui::Widget`] response
/// ([`egui::Response::clicked`]).
///
/// ```
/// use thoth_plugin_sdk::components::IconButton;
///
/// let close = IconButton::builder().icon("\u{e4f6}").tooltip("Close").build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct IconButton {
    /// Widget id used for event routing.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// The icon glyph to display (a Phosphor character).
    pub icon: String,
    /// Draw a solid frame behind the glyph. Defaults to `false`.
    #[builder(default)]
    #[serde(default)]
    pub frame: bool,
    /// Optional tooltip shown on hover.
    #[serde(default)]
    pub tooltip: Option<String>,
    /// Optional badge dot drawn in the top-right corner, as a `#rrggbb` hex
    /// colour.
    #[serde(default)]
    pub badge_color: Option<String>,
    /// Square button size preset — shares heights with [`Button`]/[`Select`], so
    /// mixed toolbars line up. Defaults to [`Size::Small`] (24px), the common
    /// compact icon-button size. Prefer this; use [`size_px`](IconButton::size_px)
    /// only for host chrome that must fit an exact pixel dimension.
    ///
    /// [`Button`]: crate::components::Button
    /// [`Select`]: crate::components::Select
    #[builder(default = Size::Small)]
    #[serde(default = "size_small")]
    pub size: Size,
    /// Exact square size in pixels, overriding [`size`](IconButton::size). An
    /// escape hatch for chrome sized to fit a specific bar; plugins should use
    /// the [`size`](IconButton::size) preset instead.
    #[serde(default)]
    pub size_px: Option<f32>,
    /// Glyph size override in pixels — derived from the size otherwise.
    #[serde(default)]
    pub icon_size: Option<f32>,
    /// Whether the button is disabled (dimmed, non-interactive).
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
    /// Whether the button is in a selected/active state (accent-coloured).
    #[builder(default)]
    #[serde(default)]
    pub selected: bool,
}
