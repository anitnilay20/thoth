#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::render_node::RenderNode;

/// A tabbed container: a header strip of labels and one [`RenderNode`] panel
/// per tab. The selected tab is kept in egui memory keyed by [`id`](Tabs::id).
///
/// `headers` and `children` are parallel: tab *i* shows `children[i]`.
///
/// ```
/// use thoth_plugin_sdk::components::Tabs;
///
/// let tabs = Tabs::builder()
///     .id("editor")
///     .headers(vec!["Request".into(), "Response".into()])
///     .build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct Tabs {
    /// Stable id salt for the selected-tab state.
    pub id: String,
    /// Tab header labels, in order.
    #[builder(default)]
    #[serde(default)]
    pub headers: Vec<String>,
    /// Optional per-tab icon glyphs (parallel to `headers`).
    #[builder(default)]
    #[serde(default)]
    pub icons: Vec<String>,
    /// Tab panels, parallel to `headers`.
    #[builder(default)]
    #[serde(default)]
    pub children: Vec<RenderNode>,
}
