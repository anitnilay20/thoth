#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::render_node::RenderNode;

/// A horizontally-scrollable, virtually-scrolled data grid with a sticky `#`
/// row-number gutter, compact headers, zebra rows, and grid lines.
///
/// Each cell is a [`RenderNode`], so cells can be plain text *or* rich nodes
/// (a `json-tree`, a `badge`, a styled run, …). Only the visible rows are laid
/// out. Render with [`show`](TableView::show), which returns the clicked row
/// index and collects cell events.
///
/// A header label of the form `"name  ·  type"` renders `name` in the header
/// weight and `type` as a small muted mono suffix.
///
/// ```
/// use thoth_plugin_sdk::components::{TableView, Typography};
/// use thoth_plugin_sdk::render_node::RenderNode;
///
/// let cell = |s: &str| RenderNode::Text(Typography::builder().text(s).build());
/// let table = TableView::builder()
///     .headers(vec!["id  ·  int".into(), "name  ·  text".into()])
///     .rows(vec![vec![cell("1"), cell("thoth")]])
///     .build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct TableView {
    /// Column header labels.
    #[builder(default)]
    #[serde(default)]
    pub headers: Vec<String>,
    /// Row data — each inner vec holds one cell node per column (padded/
    /// truncated to `headers.len()` at render time).
    #[builder(default)]
    #[serde(default)]
    pub rows: Vec<Vec<RenderNode>>,
    /// Minimum width per column in logical pixels. Defaults to 150.
    #[serde(default)]
    pub min_col_width: Option<f32>,
}
