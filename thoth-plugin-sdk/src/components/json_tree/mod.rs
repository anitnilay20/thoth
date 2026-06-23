#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An interactive, virtually-scrolled JSON tree viewer.
///
/// Renders a [`serde_json::Value`] as an expand/collapse tree with
/// syntax-coloured leaves. Expansion state is kept in egui memory keyed by
/// [`id`](JsonTree::id), so give each on-screen instance a unique id. Render
/// with [`show`](JsonTree::show).
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
    /// The JSON value to display.
    pub value: Value,
    /// Stable id salt for this instance's expansion state. Defaults to
    /// `"json-tree"` when unset (give distinct ids to multiple on-screen trees).
    #[builder(default)]
    #[serde(default)]
    pub id: String,
}
