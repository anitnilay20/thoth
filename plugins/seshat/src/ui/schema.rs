//! The schema-browser tree (lazy schema → table → columns), styled after the
//! design handoff: folder/table/view icons, table counts, and PK-keyed columns.

use serde_json::{json, Value};

use crate::state::{SchemaNode, State};
use crate::ui::widgets::{button, caret, icon, icon_sized, indent, muted};
use crate::{ICON_CIRCLE, ICON_DATABASE, ICON_EYE, ICON_FOLDER, ICON_KEY, ICON_TABLE};

pub(crate) fn schema_panel(st: &State) -> Value {
    let active = st
        .active
        .as_deref()
        .and_then(|id| st.connections.iter().find(|c| c.id == id));
    let Some(conn) = active else {
        return json!({ "type": "row", "padding": 8, "children": [
            muted("Select a connection to browse its schema.")
        ]});
    };

    let mut nodes = vec![json!({ "type": "row", "padding": 4, "children": [
        { "type": "text", "value": conn.database, "muted": true }
    ]})];

    if let Some(e) = &st.schema_error {
        nodes.push(json!({ "type": "colored", "color": "#f38ba8",
            "child": { "type": "text", "value": e, "size": "sm" } }));
    }
    if st.schemas.is_empty() && st.schema_error.is_none() {
        nodes.push(muted("Loading schemas…"));
    }

    for (i, sch) in st.schemas.iter().enumerate() {
        let count = sch
            .tables
            .as_ref()
            .map(|t| t.len().to_string())
            .unwrap_or_default();
        nodes.push(
            json!({ "type": "row", "gap": 6, "align": "fill", "children": [
                caret(&format!("sch:{i}"), sch.expanded),
                icon(ICON_FOLDER, "muted"),
                { "type": "bold", "child": { "type": "text", "value": sch.name } },
                { "type": "spacer" },
                { "type": "text", "value": count, "muted": true, "size": "sm" }
            ]}),
        );
        if sch.expanded {
            nodes.push(indent(schema_children(i, sch)));
        }
    }

    json!({ "type": "column", "gap": 2, "children": nodes })
}

fn schema_children(i: usize, sch: &SchemaNode) -> Vec<Value> {
    let Some(tables) = &sch.tables else {
        return vec![muted("Loading…")];
    };
    if tables.is_empty() {
        return vec![muted("(no tables)")];
    }
    let mut rows = Vec::new();
    for (j, tbl) in tables.iter().enumerate() {
        let (glyph, color) = match tbl.kind.as_str() {
            "view" => (ICON_EYE, "secondary"),
            "matview" => (ICON_DATABASE, "number"),
            _ => (ICON_TABLE, "string"),
        };
        // caret toggles columns; clicking the name opens a SELECT in an editor tab.
        rows.push(
            json!({ "type": "row", "gap": 6, "align": "center", "children": [
                caret(&format!("tbl:{i}:{j}"), tbl.expanded),
                icon(glyph, color),
                button(&format!("use:{i}:{j}"), &tbl.name, "Text", "Default", None, true, false)
            ]}),
        );
        if tbl.expanded {
            let cols = match &tbl.columns {
                None => vec![muted("Loading…")],
                Some(cols) if cols.is_empty() => vec![muted("(no columns)")],
                Some(cols) => cols.iter().map(column_row).collect(),
            };
            rows.push(indent(cols));
        }
    }
    rows
}

fn column_row(c: &crate::db::ColumnInfo) -> Value {
    let marker = if c.primary_key {
        icon_sized(ICON_KEY, "warning", 11.0)
    } else {
        icon_sized(ICON_CIRCLE, "muted", 7.0)
    };
    json!({ "type": "row", "gap": 6, "align": "fill", "children": [
        marker,
        { "type": "text", "value": c.name },
        { "type": "spacer" },
        { "type": "text", "value": c.data_type, "muted": true, "size": "sm" }
    ]})
}
