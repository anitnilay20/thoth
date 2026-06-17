#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::render_node::RenderNode;


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
#[derive(Debug, Clone, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct Modal {
    /// Stable id — keys the backdrop/window; unique on screen.
    pub id: String,
    /// Title shown in the modal header.
    pub title: String,
    /// Whether the modal is shown. Defaults to `false`.
    #[builder(default)]
    #[serde(default)]
    pub open: bool,
    /// Event id emitted (as a `"click"`) when the modal is closed. Falls back
    /// to `id` when unset.
    #[serde(default, rename = "close-id")]
    pub close_id: Option<String>,
    /// Width as a fraction of the viewport (0.0–1.0). When unset, sizes between
    /// 320–480px.
    #[serde(default, rename = "width-pct")]
    pub width_pct: Option<f32>,
    /// Height as a fraction of the viewport (0.0–1.0). When unset, auto-sizes.
    #[serde(default, rename = "height-pct")]
    pub height_pct: Option<f32>,
    /// Body content, rendered top-to-bottom inside the modal.
    #[builder(default)]
    #[serde(default)]
    pub children: Vec<RenderNode>,
}
