#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::components::RowHighlights;

/// An interactive, virtually-scrolled JSON tree viewer.
///
/// Reads from one of two sources:
///
/// - an inline [`serde_json::Value`], for small values a caller already holds;
/// - a **dataset handle**, resolved lazily through
///   [`dataset_access`](crate::dataset::dataset_access). Only the nodes on
///   screen are read, so this draws a file far larger than memory. Records are
///   rooted: each one is a top-level `[i]` row.
///
/// Expansion and selection are kept in egui memory keyed by [`id`](JsonTree::id),
/// so give each on-screen instance a unique id. Render with
/// [`show`](JsonTree::show).
///
/// ```
/// use thoth_plugin_sdk::components::JsonTree;
///
/// let value = serde_json::json!({ "name": "thoth", "tags": ["json", "viewer"] });
/// let tree = JsonTree::builder().value(value).id("preview").build();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct JsonTree {
    /// The JSON value to display. Ignored when [`handle`](JsonTree::handle) is set.
    #[builder(default)]
    #[serde(default)]
    pub value: Value,
    /// A dataset handle to read lazily instead of an inline value.
    #[serde(default)]
    pub handle: Option<String>,
    /// Stable id salt for this instance's expansion state. Defaults to
    /// `"json-tree"` when unset (give distinct ids to multiple on-screen trees).
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// Draw the outer container (canvas fill + hairline edge + rounded corners +
    /// 4px padding — design `.tree`). Defaults to `true`; set `false` when the
    /// tree sits inside a container that already owns those corners.
    #[builder(default = true)]
    #[serde(default = "default_true")]
    pub framed: bool,
    /// Show only these records, in this order — the shape a search result set
    /// takes. `None` shows every record.
    #[serde(default)]
    pub visible_roots: Option<Vec<u64>>,
    /// Text ranges to emphasise, keyed by node path (e.g. `"3.user.name"`).
    #[builder(default)]
    #[serde(default)]
    pub highlights: std::collections::HashMap<String, RowHighlights>,
    /// Expand every record on first show. Off by default — expanding a large
    /// file eagerly would defeat the point of reading lazily.
    #[builder(default)]
    #[serde(default)]
    pub expand_all_initially: bool,
    /// A command to carry out this frame, from the host's shortcut handling.
    #[serde(default)]
    pub action: Option<TreeAction>,
}

fn default_true() -> bool {
    true
}

/// A command the host asks a [`JsonTree`] to carry out.
///
/// Arrow keys and the context menu are handled inside the tree, but the app's
/// tree and clipboard shortcuts are user-configurable, so the host owns the
/// binding and the tree owns the behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeAction {
    /// Open the selected container.
    ExpandNode,
    /// Close the selected container.
    CollapseNode,
    /// Open every record. Costly on a large document, by nature.
    ExpandAll,
    /// Close everything, returning to a list of records.
    CollapseAll,
    /// Select the row above.
    MoveUp,
    /// Select the row below.
    MoveDown,
    /// Copy the selected node's key.
    CopyKey,
    /// Copy the selected node's value.
    CopyValue,
    /// Copy the selected node and everything under it.
    CopyObject,
    /// Copy the selected node's path.
    CopyPath,
}

/// What the user did in a [`JsonTree`], reported back to the host.
#[cfg(feature = "egui")]
#[derive(Clone, Debug, Default)]
pub struct JsonTreeOutput {
    /// Path of the currently selected node, if any.
    pub selected: Option<String>,
    /// Text the user asked to copy. The tree resolves it itself — reading the
    /// node rather than scraping the rendered row, which is truncated.
    pub copied: Option<String>,
    /// Total rows currently displayed.
    pub row_count: usize,
}
