#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::render_node::RenderNode;


/// A centered modal dialog with a dimmed backdrop.
///
/// The modal supports the SDK's two construction paths:
/// - **UI path** — call [`Modal::show`] with a closure that draws the body with
///   live widgets. This is what the host uses for native dialogs.
/// - **DSL path** — set [`body`](Modal::body) to a [`RenderNode`] tree (e.g.
///   deserialized from plugin JSON); it is rendered by walking the tree once
///   `RenderNode` rendering lands. (Today [`Modal::show`]'s closure is the only
///   active path; `body` is the serializable representation.)
///
/// Build it with the [`bon`] builder; only [`id`](Modal::id) and
/// [`title`](Modal::title) are required.
///
/// ```
/// use thoth_plugin_sdk::components::Modal;
///
/// let modal = Modal::builder().id("confirm").title("Delete file?").build();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct Modal {
    /// Stable id — used to key the backdrop/window and must be unique on screen.
    pub id: String,
    /// Title shown in the modal header.
    pub title: String,
    /// Fixed height in points. When set, the body becomes vertically
    /// scrollable; when unset, the modal sizes to its content.
    #[serde(default)]
    pub height: Option<f32>,
    /// Fixed width in points. When unset, the modal sizes between 320–480px.
    #[serde(default)]
    pub width: Option<f32>,
    /// DSL body: a [`RenderNode`] subtree rendered on the DSL path. Ignored by
    /// [`Modal::show`], which takes its body as a closure (UI path).
    #[serde(default)]
    pub body: Option<RenderNode>,
}
