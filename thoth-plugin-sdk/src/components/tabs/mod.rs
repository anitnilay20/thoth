#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::components::Size;
use crate::render_node::RenderNode;

/// A right-aligned icon action on a [`Tabs`] header line.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
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
#[non_exhaustive]
pub struct Tabs {
    /// Stable id salt for the selected-tab state.
    pub id: String,
    /// Tab header labels, in order.
    #[builder(default)]
    #[serde(default)]
    pub headers: Vec<String>,
    /// Optional per-tab icon glyphs (parallel to `headers`). A tab shows its
    /// icon *and* label when both are given; an icon with an empty header
    /// renders icon-only. See also [`icon_only`](Tabs::icon_only).
    #[builder(default)]
    #[serde(default)]
    pub icons: Vec<String>,
    /// Force icon-only tabs (labels shown as tooltips) even when headers are set.
    /// Tabs without an icon still fall back to their label. Defaults to `false`.
    #[builder(default)]
    #[serde(default, rename = "icon-only")]
    pub icon_only: bool,
    /// Header size preset. Defaults to [`Size::Medium`].
    #[builder(default)]
    #[serde(default)]
    pub size: Size,
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
