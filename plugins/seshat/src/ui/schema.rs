//! The schema-browser tree (lazy schema → table → columns), built from the
//! shared `data-row` component so it matches the file-viewer tree styling.

use thoth_plugin_sdk::components::{Colored, Column, Row, Typography, TypographyVariant};
use thoth_plugin_sdk::render_node::RenderNode;

use crate::state::{SchemaNode, State};
use crate::ui::widgets::{data_row, muted};
use crate::{ICON_CIRCLE, ICON_DATABASE, ICON_EYE, ICON_FOLDER, ICON_KEY, ICON_TABLE};

pub(crate) fn schema_panel(st: &State) -> RenderNode {
    let active = st
        .active
        .as_deref()
        .and_then(|id| st.connections.iter().find(|c| c.id == id));
    let Some(conn) = active else {
        return RenderNode::Row(
            Row::builder()
                .padding(8.0)
                .children(vec![muted("Select a connection to browse its schema.")])
                .build(),
        );
    };

    let mut nodes: Vec<RenderNode> = vec![RenderNode::Row(
        Row::builder()
            .padding(4.0)
            .children(vec![muted(&conn.database)])
            .build(),
    )];

    if let Some(e) = &st.schema_error {
        nodes.push(RenderNode::Colored(
            Colored::builder()
                .color("error")
                .child(RenderNode::Text(
                    Typography::builder()
                        .text(e.clone())
                        .variant(TypographyVariant::Caption)
                        .build(),
                ))
                .build(),
        ));
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

    RenderNode::Column(Column::builder().gap(2.0).children(nodes).build())
}

fn push_tables(nodes: &mut Vec<RenderNode>, i: usize, sch: &SchemaNode) {
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
