//! The schema-browser tree (lazy schema → table → columns), built from the
//! shared `data-row` component so it matches the file-viewer tree styling.

use thoth_plugin_sdk::components::{
    Colored, Column, DataRow, DataRowIcon, Input, Row, Scroll, Separator, Size, Spinner,
    Typography, TypographyVariant,
};
use thoth_plugin_sdk::render_node::RenderNode;
use thoth_plugin_sdk::tokens::TextToken;

use crate::db::Engine;
use crate::state::{DatabaseNode, SchemaNode, State};
use crate::ui::widgets::{data_row, muted};

/// A small inline spinner row, shown while a tree level is still loading.
fn loading_row() -> RenderNode {
    RenderNode::Row(
        Row::builder()
            .padding(6.0)
            .children(vec![RenderNode::Spinner(
                Spinner::builder().size(12.0).build(),
            )])
            .build(),
    )
}
use crate::{
    ICON_CIRCLE, ICON_DATABASE, ICON_EYE, ICON_FOLDER, ICON_KEY, ICON_TABLE, ICON_TREE_STRUCTURE,
};

pub(crate) fn schema_panel(st: &State) -> RenderNode {
    let active = st
        .active
        .as_deref()
        .and_then(|id| st.connections.iter().find(|c| c.id == id));
    if active.is_none() {
        return RenderNode::Row(
            Row::builder()
                .padding(8.0)
                .children(vec![muted("Select a connection to browse its schema.")])
                .build(),
        );
    };

    // A non-empty filter switches from the lazy tree to server-side search
    // results (matching tables across the connected database).
    let body = if st.schema_filter.trim().is_empty() {
        schema_tree(st)
    } else {
        filter_results(st)
    };

    RenderNode::Column(
        Column::builder()
            .gap(0.0)
            .children(vec![
                RenderNode::Row(
                    Row::builder()
                        .padding(6.0)
                        .children(vec![RenderNode::Input(
                            Input::builder()
                                .id("schema-filter")
                                .value(st.schema_filter.clone())
                                .placeholder("Filter tables…")
                                .grow(true)
                                .size(Size::Small)
                                .build(),
                        )])
                        .build(),
                ),
                RenderNode::Separator(Separator::plain()),
                // Vertical scroll only: rows are full-width and truncate long
                // names (with the count/action pinned right), so there's nothing
                // to scroll horizontally.
                RenderNode::Scroll(
                    Scroll::builder()
                        .id("schema-scroll")
                        .child(body)
                        .both(false)
                        .build(),
                ),
            ])
            .build(),
    )
}

/// The server-side schema-filter results: a flat list of matching tables.
fn filter_results(st: &State) -> RenderNode {
    if st.schema_searching && st.schema_matches.is_none() {
        return loading_row();
    }
    let mut nodes: Vec<RenderNode> = Vec::new();
    match &st.schema_matches {
        Some(matches) if !matches.is_empty() => {
            for (i, m) in matches.iter().enumerate() {
                let icon = if m.kind == "view" {
                    (ICON_EYE, "secondary")
                } else {
                    (ICON_TABLE, "string")
                };
                // Qualify by database so cross-database matches are distinguishable
                // (MySQL: db.table, since schema == db; Postgres: db.schema.table).
                let label = match &m.database {
                    Some(db) if *db != m.schema => format!("{db}.{}.{}", m.schema, m.name),
                    Some(db) => format!("{db}.{}", m.name),
                    None => format!("{}.{}", m.schema, m.name),
                };
                nodes.push(data_row(
                    &format!("find:{i}"),
                    &label,
                    0,
                    None,
                    Some(icon),
                    None,
                ));
            }
        }
        Some(_) => nodes.push(muted("No tables match.")),
        None => nodes.push(loading_row()),
    }
    RenderNode::Column(Column::builder().gap(2.0).children(nodes).build())
}

/// The lazy schema tree (database → schema → table/view → columns).
fn schema_tree(st: &State) -> RenderNode {
    let mut nodes: Vec<RenderNode> = Vec::new();

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
    if st.databases.is_empty() && st.schema_error.is_none() {
        nodes.push(loading_row());
    }

    // MySQL has no schema layer within a database, so render tables directly
    // under each database (skipping the redundant schema row).
    let mysql = st.engine() == Engine::Mysql;
    let limit = st.tree_limit;

    for (i, db) in st.databases.iter().enumerate().take(limit) {
        // Trailing count: for MySQL show the table count (single schema); for
        // Postgres show the schema count.
        let count = if mysql {
            db.schemas
                .as_ref()
                .and_then(|s| s.first())
                .and_then(|sc| sc.tables.as_ref())
                .map(|t| t.len().to_string())
        } else {
            db.schemas.as_ref().map(|s| s.len().to_string())
        };
        nodes.push(data_row(
            &format!("db:{i}"),
            &db.name,
            0,
            Some(db.expanded),
            Some((ICON_DATABASE, "muted")),
            count.as_deref(),
        ));
        if db.expanded {
            if mysql {
                match db.schemas.as_ref().and_then(|s| s.first()) {
                    Some(sch) => push_tables(&mut nodes, i, 0, sch, 1, limit),
                    None => nodes.push(loading_row()),
                }
            } else {
                push_schemas(&mut nodes, i, db, limit);
            }
        }
    }
    if st.databases.len() > limit {
        nodes.push(show_more_row(st.databases.len() - limit, 0));
    }

    RenderNode::Column(Column::builder().gap(2.0).children(nodes).build())
}

/// A clickable "Show more" row that reveals the next page of a capped level.
fn show_more_row(hidden: usize, indent: usize) -> RenderNode {
    data_row(
        "tree-more",
        &format!("Show {hidden} more…"),
        indent,
        None,
        Some((ICON_CIRCLE, "muted")),
        None,
    )
}

/// A table/view tree row with a trailing "view structure" action. Clicking the
/// row opens the table's data; clicking the action icon opens its structure tab.
fn table_row(
    id: &str,
    name: &str,
    indent: usize,
    expanded: bool,
    icon: (&str, &str),
) -> RenderNode {
    RenderNode::DataRow(
        DataRow::builder()
            .row_id(id)
            .display_text(name.to_string())
            .key_token(TextToken::Key)
            .indent(indent)
            .caret(expanded)
            .leading_icon(DataRowIcon::builder().glyph(icon.0).color(icon.1).build())
            .action_icon(ICON_TREE_STRUCTURE)
            .action_tooltip("View structure")
            .build(),
    )
}

fn push_schemas(nodes: &mut Vec<RenderNode>, i: usize, db: &DatabaseNode, limit: usize) {
    let Some(schemas) = &db.schemas else {
        nodes.push(loading_row());
        return;
    };
    if schemas.is_empty() {
        nodes.push(muted("(no schemas)"));
        return;
    }
    for (j, sch) in schemas.iter().enumerate() {
        let count = sch.tables.as_ref().map(|t| t.len().to_string());
        nodes.push(data_row(
            &format!("sch:{i}:{j}"),
            &sch.name,
            1,
            Some(sch.expanded),
            Some((ICON_FOLDER, "muted")),
            count.as_deref(),
        ));
        if sch.expanded {
            push_tables(nodes, i, j, sch, 2, limit);
        }
    }
}

fn push_tables(
    nodes: &mut Vec<RenderNode>,
    i: usize,
    j: usize,
    sch: &SchemaNode,
    indent: usize,
    limit: usize,
) {
    let Some(tables) = &sch.tables else {
        nodes.push(loading_row());
        return;
    };
    if tables.is_empty() {
        nodes.push(muted("(no tables)"));
        return;
    }
    for (k, tbl) in tables.iter().enumerate().take(limit) {
        let icon = match tbl.kind.as_str() {
            "view" => (ICON_EYE, "secondary"),
            "matview" => (ICON_DATABASE, "number"),
            _ => (ICON_TABLE, "string"),
        };
        nodes.push(table_row(
            &format!("tbl:{i}:{j}:{k}"),
            &tbl.name,
            indent,
            tbl.expanded,
            icon,
        ));
        if tbl.expanded {
            match &tbl.columns {
                None => nodes.push(loading_row()),
                Some(cols) if cols.is_empty() => nodes.push(muted("(no columns)")),
                Some(cols) => {
                    for (l, c) in cols.iter().enumerate() {
                        let marker = if c.primary_key {
                            (ICON_KEY, "warning")
                        } else {
                            (ICON_CIRCLE, "muted")
                        };
                        nodes.push(data_row(
                            &format!("col:{i}:{j}:{k}:{l}"),
                            &c.name,
                            indent + 1,
                            None,
                            Some(marker),
                            Some(&c.data_type),
                        ));
                    }
                }
            }
        }
    }
    if tables.len() > limit {
        nodes.push(show_more_row(tables.len() - limit, indent));
    }
}
