#[cfg(feature = "egui")]
pub(crate) mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::render_node::RenderNode;

fn default_true() -> bool {
    true
}

/// A horizontally-scrollable, virtually-scrolled data grid with a sticky `#`
/// row-number gutter, compact headers, zebra rows, and grid lines. Columns can
/// be resized by dragging their edge, and auto-fitted to their visible content
/// (never below `min_col_width`) from a header's right-click menu — or by
/// double-clicking the header, unless [`sortable`](TableView::sortable) has
/// claimed the click for sorting.
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
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
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
    /// Minimum width per column in logical pixels. Defaults to 150. A column
    /// may grow beyond this value through resizing or header auto-fit, but can
    /// never be dragged narrower.
    #[serde(default)]
    pub min_col_width: Option<f32>,
    /// Optional column type per column, parallel to [`headers`](TableView::headers).
    /// Drives per-type cell styling/alignment (numeric + temporal right-align).
    /// Empty (or a shorter vec) leaves those columns rendered exactly as before.
    #[builder(default)]
    #[serde(default)]
    pub column_types: Vec<ColumnType>,
    /// Draw the outer container (canvas fill + hairline edge + rounded corners,
    /// rows clipped to them). Defaults to `true`; set `false` when the grid sits
    /// inside a container that already owns those corners — [`DataView`] draws
    /// the grid flush and border-less inside its own frame.
    ///
    /// [`DataView`]: crate::components::DataView
    #[builder(default = true)]
    #[serde(default = "default_true")]
    pub framed: bool,
    /// Which columns the rows are currently ordered by, in order — each drawn
    /// as a direction arrow in its header. The grid never reorders anything
    /// itself: it shows a page of a result that may be far larger than the
    /// page, and sorting only the page would order the wrong rows.
    #[builder(default)]
    #[serde(default, deserialize_with = "deserialize_sorts")]
    pub sort: Vec<SortBy>,
    /// Offer sorting at all. When set, clicking a header emits
    /// [`SORT_COLUMN`](crate::actions::SORT_COLUMN) carrying the sort the
    /// click moves to (see [`TableView::next_sort`]), and it is the producer's
    /// job to re-run the query and hand back the reordered rows.
    ///
    /// Off by default: a grid whose rows came from somewhere unsortable — a
    /// plugin's own page, a text index — must not appear to offer it.
    #[builder(default)]
    #[serde(default)]
    pub sortable: bool,
}

/// How a grid is ordered: one column, one direction.
///
/// Serialized as the [`SORT_COLUMN`](crate::actions::SORT_COLUMN) event's
/// value, where `null` in place of it means the sort was cleared. `append`
/// marks a Shift-click: the producer is asked to add the column to the order
/// it already holds rather than replace that order with it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortBy {
    /// The column's header name — as [`TableView::headers`] spells it, with
    /// any `"  ·  type"` suffix stripped.
    pub column: String,
    /// Largest first.
    #[serde(default)]
    pub descending: bool,
    /// Add this column to the order rather than replacing it with it. Set when
    /// the header click that produced this sort was a Shift-click; the producer
    /// — which holds the whole multi-key order — decides whether the column is
    /// being appended or cycled out. Ignored when a `SortBy` merely *displays*
    /// the current order.
    #[serde(default, skip_serializing_if = "is_false")]
    pub append: bool,
}

/// `serde(skip_serializing_if)` helper: `append` is only worth spelling when it
/// is true, so a plain click keeps the short `{"column","descending"}` shape.
fn is_false(value: &bool) -> bool {
    !*value
}

/// Deserialize a [`TableView::sort`] that was written by an older SDK as an
/// `Option<SortBy>` — `null` when unsorted, a single object when one column was
/// marked — as well as the current list form. A payload that only ever knew one
/// arrow keeps working; nothing errors, it just now has room for several.
pub(crate) fn deserialize_sorts<'de, D>(de: D) -> Result<Vec<SortBy>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Raw {
        One(SortBy),
        Many(Vec<SortBy>),
        Null,
    }

    Ok(match Raw::deserialize(de)? {
        Raw::One(sort) => vec![sort],
        Raw::Many(sorts) => sorts,
        Raw::Null => Vec::new(),
    })
}

impl TableView {
    /// The sort a click on `column` moves to, given `current` — the *primary*
    /// sort key: a fresh column starts ascending, a second click reverses it,
    /// and a third clears it.
    ///
    /// Three states rather than two because a cleared sort is the only way
    /// back to the order the file itself has, which no direction can express.
    pub fn next_sort(current: Option<&SortBy>, column: &str) -> Option<SortBy> {
        match current {
            Some(sort) if sort.column == column => (!sort.descending).then(|| SortBy {
                column: column.to_string(),
                descending: true,
                ..Default::default()
            }),
            _ => Some(SortBy {
                column: column.to_string(),
                descending: false,
                ..Default::default()
            }),
        }
    }
}

/// A header label without its `"name  ·  type"` annotation — the name alone is
/// what a sort names, and what the producer knows the column by.
pub(crate) fn header_name(label: &str) -> &str {
    label.split_once("  ·  ").map_or(label, |(name, _)| name)
}

impl Default for TableView {
    /// An empty, framed grid — mirrors the builder's defaults.
    fn default() -> Self {
        Self::builder().build()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn asc(column: &str) -> SortBy {
        SortBy {
            column: column.to_string(),
            descending: false,
            ..Default::default()
        }
    }

    fn desc(column: &str) -> SortBy {
        SortBy {
            column: column.to_string(),
            descending: true,
            ..Default::default()
        }
    }

    #[test]
    fn a_column_cycles_ascending_then_descending_then_off() {
        let first = TableView::next_sort(None, "ts");
        assert_eq!(first, Some(asc("ts")));
        let second = TableView::next_sort(first.as_ref(), "ts");
        assert_eq!(second, Some(desc("ts")));
        // The third click leaves the file's own order, which no direction says.
        assert_eq!(TableView::next_sort(second.as_ref(), "ts"), None);
    }

    #[test]
    fn another_column_starts_its_own_cycle_rather_than_continuing_this_one() {
        // Without this, clicking a second column while the first is descending
        // would open it descending — a direction the user never asked for.
        assert_eq!(
            TableView::next_sort(Some(&desc("ts")), "level"),
            Some(asc("level"))
        );
    }

    #[test]
    fn a_cleared_sort_serializes_as_null() {
        // The event value is the whole `Option`, so "no sort" has a spelling
        // and the producer is never left guessing from an empty string.
        let cleared: Option<SortBy> = None;
        assert_eq!(serde_json::to_string(&cleared).unwrap(), "null");
        assert_eq!(
            serde_json::to_string(&Some(desc("ts"))).unwrap(),
            r#"{"column":"ts","descending":true}"#
        );
        assert_eq!(
            serde_json::from_str::<Option<SortBy>>("null").unwrap(),
            None
        );
    }

    #[test]
    fn a_sort_names_the_column_without_its_type_annotation() {
        // Headers carry `"name  ·  type"`; the producer knows only `name`.
        assert_eq!(header_name("ts  ·  TIMESTAMP"), "ts");
        assert_eq!(header_name("level"), "level");
    }

    #[test]
    fn a_shift_click_marks_the_sort_as_append() {
        // A plain click replaces the order; a Shift-click asks to add to it.
        // The flag is spelled only when it is true, so a plain click keeps the
        // same shape it always had.
        assert_eq!(
            serde_json::to_string(&Some(desc("ts"))).unwrap(),
            r#"{"column":"ts","descending":true}"#
        );
        assert_eq!(
            serde_json::to_string(&Some(SortBy {
                column: "ts".to_string(),
                descending: false,
                append: true,
            }))
            .unwrap(),
            r#"{"column":"ts","descending":false,"append":true}"#
        );
        // And it round-trips, defaulting to false when absent.
        let plain: SortBy = serde_json::from_str(r#"{"column":"ts"}"#).unwrap();
        assert!(!plain.append);
        let appended: SortBy = serde_json::from_str(r#"{"column":"ts","append":true}"#).unwrap();
        assert!(appended.append);
    }

    #[test]
    fn an_old_single_sort_deserializes_into_the_list() {
        // An older SDK wrote `sort` as `Option<SortBy>`: `null` when unsorted,
        // one object otherwise. The new `Vec` reads both without error.
        #[derive(serde::Deserialize)]
        struct Holder {
            #[serde(default, deserialize_with = "deserialize_sorts")]
            sort: Vec<SortBy>,
        }

        let null: Holder = serde_json::from_str(r#"{"sort":null}"#).unwrap();
        assert!(null.sort.is_empty());

        let one: Holder =
            serde_json::from_str(r#"{"sort":{"column":"ts","descending":true}}"#).unwrap();
        assert_eq!(one.sort, vec![desc("ts")]);

        let many: Holder =
            serde_json::from_str(r#"{"sort":[{"column":"a"},{"column":"b","descending":true}]}"#)
                .unwrap();
        assert_eq!(many.sort.len(), 2);
        assert_eq!(many.sort[1].column, "b");
    }
}
