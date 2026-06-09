//! The schema-browser tree (lazy schema → table → columns), built from the
//! shared `data-row` component so it matches the file-viewer tree styling.

use serde_json::{json, Value};

use crate::state::{SchemaNode, State};
use crate::ui::widgets::{data_row, muted};
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
        let count = sch.tables.as_ref().map(|t| t.len().to_string());
        nodes.push(data_row(
            &format!("sch:{i}"),
            &sch.name,
            0,
            Some(sch.expanded),
            Some((ICON_FOLDER, "muted")),
            count.as_deref(),
        ));
        if sch.expanded {
            push_tables(&mut nodes, i, sch);
        }
    }

    json!({ "type": "column", "gap": 2, "children": nodes })
}

fn push_tables(nodes: &mut Vec<Value>, i: usize, sch: &SchemaNode) {
    let Some(tables) = &sch.tables else {
        nodes.push(muted("Loading…"));
        return;
    };
    if tables.is_empty() {
        nodes.push(muted("(no tables)"));
        return;
    }
    for (j, tbl) in tables.iter().enumerate() {
        let icon = match tbl.kind.as_str() {
            "view" => (ICON_EYE, "secondary"),
            "matview" => (ICON_DATABASE, "number"),
            _ => (ICON_TABLE, "string"),
        };
        // caret → toggle columns; row click → open a SELECT in an editor tab.
        nodes.push(data_row(
            &format!("tbl:{i}:{j}"),
            &tbl.name,
            1,
            Some(tbl.expanded),
            Some(icon),
            None,
        ));
        if tbl.expanded {
            match &tbl.columns {
                None => nodes.push(muted("Loading…")),
                Some(cols) if cols.is_empty() => nodes.push(muted("(no columns)")),
                Some(cols) => {
                    for (k, c) in cols.iter().enumerate() {
                        let marker = if c.primary_key {
                            (ICON_KEY, "warning")
                        } else {
                            (ICON_CIRCLE, "muted")
                        };
                        nodes.push(data_row(
                            &format!("col:{i}:{j}:{k}"),
                            &c.name,
                            2,
                            None,
                            Some(marker),
                            Some(&c.data_type),
                        ));
                    }
                }
            }
        }
    }
}
