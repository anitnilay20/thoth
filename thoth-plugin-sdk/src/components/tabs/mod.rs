#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::render_node::RenderNode;

/// A right-aligned icon action on a [`Tabs`] header line.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct TabAction {
    /// Event id emitted (as a "click") when the action is pressed.
    pub id: String,
    /// The action's Phosphor glyph.
    pub icon: String,
    /// Optional tooltip shown on hover.
    #[serde(default)]
    pub tooltip: Option<String>,
}

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
    /// Optional per-tab icon glyphs (parallel to `headers`). When a tab's glyph
    /// is set, it renders as an icon-only tab with the header text as tooltip.
    #[builder(default)]
    #[serde(default)]
    pub icons: Vec<String>,
    /// Right-aligned icon actions on the header line.
    #[builder(default)]
    #[serde(default)]
    pub actions: Vec<TabAction>,
    /// Tab panels, parallel to `headers`.
    #[builder(default)]
    #[serde(default)]
    pub children: Vec<RenderNode>,
    /// Gap (px) between the tab strip and the panel content below it. Defaults
    /// to 10; set to 0 for a panel that sits flush under the tabs.
    #[serde(default, rename = "content-gap")]
    pub content_gap: Option<f32>,
}
