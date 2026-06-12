//! The SQL editor tab: header, code editor, Run, and the typed results grid.

use serde_json::{json, Value};

use crate::state::State;
use crate::ui::widgets::button;
use crate::ICON_PLAY;

pub(crate) fn editor_view(st: &State) -> Value {
    json!({
        "type": "column", "gap": 0, "children": [
            { "type": "row", "padding": 8, "gap": 8, "children": [
                button("run", "Run", "Elevated", "Primary", Some(ICON_PLAY), !st.loading, false)
            ]},
            { "type": "separator" },
            { "type": "code-editor", "id": "sql", "value": st.sql, "font-size": 12.0, "syntax": "sql" },
            { "type": "separator" },
            { "type": "scroll", "id": "results-scroll", "child": results(st) }
        ]
    })
}

fn results(st: &State) -> Value {
    if st.loading {
        return json!({ "type": "row", "padding": 16, "gap": 10, "align": "center", "children": [
            { "type": "spinner" },
            { "type": "text", "muted": true, "value": "Running query\u{2026}" }
        ]});
    }
    match &st.result {
        Some(Ok(result)) => results_table(result),
        Some(Err(msg)) => json!({ "type": "row", "padding": 12, "children": [
            { "type": "colored", "color": "#f38ba8",
              "child": { "type": "text", "value": format!("Error: {msg}") } }
        ]}),
        None => json!({ "type": "row", "padding": 12, "children": [
            { "type": "text", "muted": true, "value": "Run a query to see results." }
        ]}),
    }
}

/// Render a `QueryResult` ({columns, rows, tag}) as a typed table, or — for a
/// statement with no result set — its command tag.
fn results_table(result: &Value) -> Value {
    let columns = result.get("columns").and_then(|c| c.as_array());
    let rows = result.get("rows").and_then(|r| r.as_array());
    let tag = result.get("tag").and_then(|t| t.as_str());

    match (columns, rows) {
        (Some(cols), Some(rows)) if !cols.is_empty() => {
            let headers: Vec<Value> = cols
                .iter()
                .map(|c| {
                    let name = c.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let ty = c.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    json!(if ty.is_empty() {
                        name.to_string()
                    } else {
                        format!("{name}  ·  {ty}")
                    })
                })
                .collect();
            let table_rows: Vec<Value> = rows
                .iter()
                .map(|row| {
                    let cells: Vec<Value> = row
                        .as_array()
                        .map(|cs| cs.iter().map(cell_node).collect())
                        .unwrap_or_default();
                    Value::Array(cells)
                })
                .collect();

            let footer = format!(
                "{} row{}{}",
                rows.len(),
                if rows.len() == 1 { "" } else { "s" },
                tag.map(|t| format!("  ·  {t}")).unwrap_or_default()
            );
            json!({ "type": "column", "gap": 4, "children": [
                { "type": "table", "headers": headers, "rows": table_rows },
                { "type": "row", "padding": 6, "children": [
                    { "type": "text", "muted": true, "value": footer }
                ]}
            ]})
        }
        _ => json!({ "type": "row", "padding": 12, "children": [
            { "type": "text", "muted": true, "value": tag.unwrap_or("Query OK").to_string() }
        ]}),
    }
}

/// Map a single typed cell value to a display node: NULL muted, JSON/JSONB as an
/// interactive tree, scalars as text.
fn cell_node(value: &Value) -> Value {
    match value {
        Value::Null => {
            json!({ "type": "italic", "child": { "type": "text", "value": "NULL", "muted": true } })
        }
        Value::Object(_) | Value::Array(_) => json!({ "type": "json-tree", "value": value }),
        Value::Bool(b) => json!({ "type": "text", "value": b.to_string() }),
        Value::Number(n) => json!({ "type": "text", "value": n.to_string() }),
        Value::String(s) => json!({ "type": "text", "value": s }),
    }
}
