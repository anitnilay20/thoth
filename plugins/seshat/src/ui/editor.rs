//! The SQL editor tab: header, code editor, Run, and the typed results grid.

use serde_json::Value;
use thoth_plugin_sdk::components::{
    CodeEditor, Colored, Column, JsonTree, Row, Scroll, Separator, Spinner, TableView, Typography,
    TypographyVariant,
};
use thoth_plugin_sdk::render_node::RenderNode;

use crate::state::State;
use crate::ui::widgets::{button, muted};
use crate::ICON_PLAY;

pub(crate) fn editor_view(st: &State) -> RenderNode {
    RenderNode::Column(
        Column::builder()
            .gap(0.0)
            .children(vec![
                RenderNode::Row(
                    Row::builder()
                        .padding(8.0)
                        .gap(8.0)
                        .children(vec![button(
                            "run",
                            "Run",
                            "Elevated",
                            "Primary",
                            Some(ICON_PLAY),
                            !st.loading,
                            false,
                        )])
                        .build(),
                ),
                RenderNode::Separator(Separator::plain()),
                RenderNode::CodeEditor(
                    CodeEditor::builder()
                        .id("sql")
                        .value(st.sql.clone())
                        .font_size(12.0)
                        .syntax("sql")
                        .bordered(false)
                        .build(),
                ),
                RenderNode::Separator(Separator::plain()),
                RenderNode::Scroll(Scroll::builder().child(results(st)).build()),
            ])
            .build(),
    )
}

fn results(st: &State) -> RenderNode {
    if st.loading {
        return RenderNode::Row(
            Row::builder()
                .padding(16.0)
                .gap(10.0)
                .align(thoth_plugin_sdk::components::Align::Center)
                .children(vec![
                    RenderNode::Spinner(Spinner::builder().build()),
                    muted("Running query…"),
                ])
                .build(),
        );
    }
    match &st.result {
        Some(Ok(result)) => results_table(result),
        Some(Err(msg)) => RenderNode::Row(
            Row::builder()
                .padding(12.0)
                .children(vec![RenderNode::Colored(
                    Colored::builder()
                        .color("#f38ba8")
                        .child(RenderNode::Text(
                            Typography::builder().text(format!("Error: {msg}")).build(),
                        ))
                        .build(),
                )])
                .build(),
        ),
        None => RenderNode::Row(
            Row::builder()
                .padding(12.0)
                .children(vec![muted("Run a query to see results.")])
                .build(),
        ),
    }
}

/// Render a `QueryResult` ({columns, rows, tag}) as a typed table, or — for a
/// statement with no result set — its command tag.
fn results_table(result: &Value) -> RenderNode {
    let columns = result.get("columns").and_then(|c| c.as_array());
    let rows = result.get("rows").and_then(|r| r.as_array());
    let tag = result.get("tag").and_then(|t| t.as_str());

    match (columns, rows) {
        (Some(cols), Some(rows)) if !cols.is_empty() => {
            let headers: Vec<String> = cols
                .iter()
                .map(|c| {
                    let name = c.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let ty = c.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if ty.is_empty() {
                        name.to_string()
                    } else {
                        format!("{name}  ·  {ty}")
                    }
                })
                .collect();
            let table_rows: Vec<Vec<RenderNode>> = rows
                .iter()
                .map(|row| {
                    row.as_array()
                        .map(|cs| cs.iter().map(cell_node).collect())
                        .unwrap_or_default()
                })
                .collect();

            let footer = format!(
                "{} row{}{}",
                rows.len(),
                if rows.len() == 1 { "" } else { "s" },
                tag.map(|t| format!("  ·  {t}")).unwrap_or_default()
            );
            RenderNode::Column(
                Column::builder()
                    .gap(4.0)
                    .children(vec![
                        RenderNode::Table(
                            TableView::builder().headers(headers).rows(table_rows).build(),
                        ),
                        RenderNode::Row(
                            Row::builder().padding(6.0).children(vec![muted(&footer)]).build(),
                        ),
                    ])
                    .build(),
            )
        }
        _ => RenderNode::Row(
            Row::builder()
                .padding(12.0)
                .children(vec![muted(tag.unwrap_or("Query OK"))])
                .build(),
        ),
    }
}

/// Map a typed cell value to a display node: NULL muted-italic, JSON/JSONB as an
/// interactive tree, scalars as text.
fn cell_node(value: &Value) -> RenderNode {
    match value {
        Value::Null => RenderNode::Text(
            Typography::builder()
                .text("NULL")
                .italic(true)
                .variant(TypographyVariant::BodyMuted)
                .build(),
        ),
        Value::Object(_) | Value::Array(_) => {
            RenderNode::JsonTree(JsonTree::builder().value(value.clone()).build())
        }
        Value::Bool(b) => RenderNode::Text(Typography::builder().text(b.to_string()).build()),
        Value::Number(n) => RenderNode::Text(Typography::builder().text(n.to_string()).build()),
        Value::String(s) => RenderNode::Text(Typography::builder().text(s.clone()).build()),
    }
}
