#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

/// A horizontally-scrollable, virtually-scrolled data grid with a sticky `#`
/// row-number gutter, compact headers, zebra rows, and grid lines.
///
/// Data-driven: rows are plain text cells (`Vec<Vec<String>>`), so the table is
/// serializable and only the visible rows are laid out. Render with
/// [`show`](TableView::show), which returns the clicked row index.
///
/// A header label of the form `"name  ·  type"` renders `name` in the header
/// weight and `type` as a small muted mono suffix.
///
/// ```
/// use thoth_plugin_sdk::components::TableView;
///
/// let table = TableView::builder()
///     .headers(vec!["id  ·  int".into(), "name  ·  text".into()])
///     .rows(vec![
///         vec!["1".into(), "thoth".into()],
///         vec!["2".into(), "seshat".into()],
///     ])
///     .build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct TableView {
    /// Column header labels.
    #[builder(default)]
    #[serde(default)]
    pub headers: Vec<String>,
    /// Row data — each inner vec holds one cell per column (padded/truncated to
    /// `headers.len()` at render time).
    #[builder(default)]
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
    /// Minimum width per column in logical pixels. Defaults to 150.
    #[serde(default)]
    pub min_col_width: Option<f32>,
}
