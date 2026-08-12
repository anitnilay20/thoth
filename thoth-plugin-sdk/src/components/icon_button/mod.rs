#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::components::Size;

fn size_small() -> Size {
    Size::Small
}

/// How an [`IconButton`] paints its [`selected`](IconButton::selected) state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IconButtonSelectedStyle {
    /// Design `.ib.active`: a solid accent slab with a contrast glyph and a
    /// matching glow. The default, and what every existing caller gets.
    #[default]
    Solid,
    /// Design `.bell`: a 14% accent wash under an `fg` glyph, with no glow —
    /// an "this popover is open" hint rather than a pressed toolbar toggle.
    Wash,
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
    /// Draw a solid frame behind the glyph — design `.ib`: a surface fill with a
    /// hairline edge. Defaults to `false`, the design's `.ib.ghost` variant, which
    /// only fills on hover.
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
    /// Badge dot diameter in points. Defaults to 9 — design `.ib .bdot`. The
    /// design's `.bell .bd` is an 8px dot instead.
    #[serde(default, rename = "badge-size")]
    pub badge_size: Option<f32>,
    /// How far the dot sits *inside* the top-right corner, in points. Defaults
    /// to `-2`, i.e. the design's `.bdot{top:-2px;right:-2px}` overhang; the
    /// design's `.bell .bd{top:3px;right:3px}` tucks it in at `3`.
    #[serde(default, rename = "badge-inset")]
    pub badge_inset: Option<f32>,
    /// Colour of the 2pt ring around the badge dot (hex or theme token) — the
    /// dot is ringed in whatever it sits on so it detaches from the glyph.
    /// Defaults to the canvas background; a button on a panel wants the panel's.
    #[serde(default, rename = "badge-ring-color")]
    pub badge_ring_color: Option<String>,
    /// Glyph colour override (hex or theme token). When unset the glyph follows
    /// the button's state, which is what nearly every caller wants; the design's
    /// `.bell` pins it to `fg` in every state instead.
    #[serde(default, rename = "glyph-color")]
    pub glyph_color: Option<String>,
    /// Square button size preset — shares heights with [`Button`] (via
    /// [`Size::metrics`]), so a toolbar of buttons and icon buttons lines up.
    /// Defaults to [`Size::Small`] (22px), the common compact icon-button size.
    /// Note these heights are deliberately shorter than a text-entry control's
    /// ([`Size::field_metrics`], which [`Select`] uses). Prefer this prop; use
    /// [`size_px`](IconButton::size_px) only for host chrome that must fit an
    /// exact pixel dimension.
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
    /// How [`selected`](IconButton::selected) is painted. Defaults to
    /// [`IconButtonSelectedStyle::Solid`], the existing accent slab + glow.
    #[builder(default)]
    #[serde(default, rename = "selected-style")]
    pub selected_style: IconButtonSelectedStyle,
}
