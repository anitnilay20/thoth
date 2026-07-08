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
#[non_exhaustive]
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
    /// Optional column type per column, parallel to [`headers`](TableView::headers).
    /// Drives per-type cell styling/alignment (numeric + temporal right-align).
    /// Empty (or a shorter vec) leaves those columns rendered exactly as before.
    #[builder(default)]
    #[serde(default)]
    pub column_types: Vec<ColumnType>,
}

/// The supported column formats a [`TableView`] styles cells by. Map a raw SQL
/// type name with [`ColumnType::from_sql`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnType {
    /// Plain text / unknown — rendered as-is (the default).
    #[default]
    Text,
    /// Whole numbers — number colour, right-aligned.
    Integer,
    /// Fractional / decimal numbers — number colour, right-aligned.
    Float,
    /// Booleans — boolean colour.
    Boolean,
    /// Date + time — temporal tint, mono, right-aligned.
    Timestamp,
    /// Calendar date — temporal tint, mono, right-aligned.
    Date,
    /// Time of day — temporal tint, mono, right-aligned.
    Time,
    /// UUID — mono, muted.
    Uuid,
    /// JSON / JSONB — rendered as an interactive tree (or text).
    Json,
    /// Enumerated type — values shown as a coloured pill.
    Enum,
}

impl ColumnType {
    /// Classify a raw SQL type name (engine-agnostic, by substring) — recognises
    /// the common Postgres/MySQL spellings; unknown types fall back to [`Text`].
    ///
    /// [`Text`]: ColumnType::Text
    pub fn from_sql(sql_type: &str) -> ColumnType {
        let t = sql_type.to_ascii_lowercase();
        if t.contains("enum") {
            ColumnType::Enum
        } else if t.contains("timestamp") || t.contains("datetime") {
            ColumnType::Timestamp
        } else if t.contains("date") {
            ColumnType::Date
        } else if t.contains("time") {
            ColumnType::Time
        } else if t.contains("bool") {
            ColumnType::Boolean
        } else if t.contains("uuid") {
            ColumnType::Uuid
        } else if t.contains("json") {
            ColumnType::Json
        } else if t.contains("int") || t.contains("serial") {
            ColumnType::Integer
        } else if t.contains("numeric")
            || t.contains("decimal")
            || t.contains("real")
            || t.contains("double")
            || t.contains("float")
            || t.contains("money")
        {
            ColumnType::Float
        } else {
            ColumnType::Text
        }
    }

    /// Numeric and temporal values read best right-aligned in a grid.
    pub fn right_aligned(self) -> bool {
        matches!(
            self,
            ColumnType::Integer
                | ColumnType::Float
                | ColumnType::Timestamp
                | ColumnType::Date
                | ColumnType::Time
        )
    }

    /// The semantic theme colour token for values of this type, matching the
    /// design handoff's result-grid cells: numbers use the number syntax colour,
    /// dates/times the string syntax colour, and everything else the default
    /// foreground. `Enum`/`Json` render specially (a pill / a tree).
    pub fn text_color(self) -> &'static str {
        match self {
            ColumnType::Integer | ColumnType::Float => "number",
            ColumnType::Timestamp | ColumnType::Date | ColumnType::Time => "string",
            _ => "fg",
        }
    }
}
