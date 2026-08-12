#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::render_node::RenderNode;

/// `serde` default for [`Modal::dismissible`], which defaults to `true`.
fn default_true() -> bool {
    true
}

/// A centered modal overlay dialog with a dimmed backdrop.
///
/// Visibility is plugin-controlled via [`open`](Modal::open): the host only
/// renders the overlay when `open` is true. Closing it (Escape, backdrop click,
/// or the header close button) emits a `"click"` event with
/// [`close_id`](Modal::close_id) (falling back to [`id`](Modal::id)).
///
/// Content is its [`children`](Modal::children) (the DSL path). For native host
/// use, [`Modal::show_with`] takes a closure instead.
///
/// ```
/// use thoth_plugin_sdk::components::Modal;
///
/// let modal = Modal::builder().id("confirm").title("Delete file?").open(true).build();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Modal {
    /// Stable id — keys the backdrop/window; unique on screen.
    pub id: String,
    /// Title shown in the modal header.
    pub title: String,
    /// Optional tiny label *above* the title — design `.grouplabel`, the
    /// header's eyebrow ("PERMISSION REQUESTED"). Text is used verbatim, so pass
    /// it already upper-cased if that is the intent.
    #[serde(default)]
    pub eyebrow: Option<String>,
    /// Colour for [`eyebrow`](Modal::eyebrow) as a theme token or hex (e.g.
    /// `"warning"` for design `.grouplabel.warn`). Defaults to the group-label
    /// muted foreground.
    #[serde(default, rename = "eyebrow-color")]
    pub eyebrow_color: Option<String>,
    /// Optional secondary line under the title (e.g. a step hint).
    #[serde(default)]
    pub subtitle: Option<String>,
    /// Whether the modal is shown. Defaults to `false`.
    #[builder(default)]
    #[serde(default)]
    pub open: bool,
    /// Event id emitted (as a `"click"`) when the modal is closed. Falls back
    /// to `id` when unset.
    #[serde(default, rename = "close-id")]
    pub close_id: Option<String>,
    /// Fixed width in pixels. Takes precedence over [`width_pct`](Modal::width_pct).
    #[serde(default)]
    pub width: Option<f32>,
    /// Width as a fraction of the viewport (0.0–1.0). When unset, sizes between
    /// 320–480px.
    #[serde(default, rename = "width-pct")]
    pub width_pct: Option<f32>,
    /// Height as a fraction of the viewport (0.0–1.0). When unset, auto-sizes.
    #[serde(default, rename = "height-pct")]
    pub height_pct: Option<f32>,
    /// Optional leading glyph in the header, before the title — design
    /// `.m-head`'s status glyph / avatar slot. A Phosphor glyph string.
    #[serde(default)]
    pub glyph: Option<String>,
    /// Colour for [`glyph`](Modal::glyph) as a theme token name (e.g. `"error"`,
    /// `"warning"`, `"info"`, `"accent"`). Defaults to `accent`.
    #[serde(default, rename = "glyph-color")]
    pub glyph_color: Option<String>,
    /// Render the glyph as a filled 44px tile with a hairline edge (design
    /// `.avatar`) rather than a bare glyph (design `.glyph`). Defaults to `false`.
    #[builder(default)]
    #[serde(default)]
    pub glyph_tile: bool,
    /// Whether the header shows a close (✕) affordance and Escape / backdrop
    /// clicks dismiss the modal. Defaults to `true`; set `false` for a dialog the
    /// user must answer (the design's update-consent card has no `.x`).
    #[builder(default = true)]
    #[serde(default = "crate::components::modal::default_true")]
    pub dismissible: bool,
    /// Body content, rendered top-to-bottom inside the modal.
    #[builder(default)]
    #[serde(default)]
    pub children: Vec<RenderNode>,
    /// Action bar pinned to the bottom of the card — design `.m-foot`: a
    /// full-bleed panel strip with a hairline along its top edge, laid out
    /// right-to-left so the first node ends up rightmost.
    ///
    /// This is the DSL equivalent of [`Modal::show_with_footer`], so a plugin gets
    /// the same footer chrome the host does instead of approximating it with a
    /// separator and a row inside the body.
    #[builder(default)]
    #[serde(default)]
    pub footer: Vec<RenderNode>,
}

impl Default for Modal {
    /// A closed, dismissible modal — mirrors the builder's and the
    /// deserializer's defaults (a derived `Default` would leave
    /// [`dismissible`](Modal::dismissible) `false`).
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            eyebrow: None,
            eyebrow_color: None,
            subtitle: None,
            open: false,
            close_id: None,
            width: None,
            width_pct: None,
            height_pct: None,
            glyph: None,
            glyph_color: None,
            glyph_tile: false,
            dismissible: default_true(),
            children: Vec::new(),
            footer: Vec::new(),
        }
    }
}
