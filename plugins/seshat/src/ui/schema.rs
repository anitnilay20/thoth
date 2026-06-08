//! The schema-browser tree (lazy schema → table → columns).

use serde_json::{json, Value};

use crate::state::{SchemaNode, State};
use crate::ui::widgets::{button, caret, indent, muted};

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
        { "type": "text", "value": conn.name, "muted": true }
    ]})];

    if let Some(e) = &st.schema_error {
        nodes.push(json!({ "type": "colored", "color": "#f38ba8",
            "child": { "type": "text", "value": e, "size": "sm" } }));
    }
    if st.schemas.is_empty() && st.schema_error.is_none() {
        nodes.push(muted("Loading schemas…"));
    }

    for (i, sch) in st.schemas.iter().enumerate() {
        nodes.push(
            json!({ "type": "row", "gap": 4, "align": "center", "children": [
                caret(&format!("sch:{i}"), sch.expanded),
                { "type": "text", "value": sch.name }
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
        // caret toggles columns; clicking the name opens a SELECT in an editor tab.
        let mut row_children = vec![
            caret(&format!("tbl:{i}:{j}"), tbl.expanded),
            button(
                &format!("use:{i}:{j}"),
                &tbl.name,
                "Text",
                "Default",
                None,
                true,
                false,
            ),
        ];
        if tbl.kind == "view" {
            row_children.push(muted("view"));
        }
        rows.push(json!({ "type": "row", "gap": 4, "align": "center", "children": row_children }));
        if tbl.expanded {
            let cols = match &tbl.columns {
                None => vec![muted("Loading…")],
                Some(cols) if cols.is_empty() => vec![muted("(no columns)")],
                Some(cols) => cols
                    .iter()
                    .map(|c| {
                        let pk = if c.primary_key { "  ·  PK" } else { "" };
                        muted(&format!("{}  {}{}", c.name, c.data_type, pk))
                    })
                    .collect(),
            };
            rows.push(indent(cols));
        }
    }
    rows
}
