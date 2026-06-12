//! The query-history list.

use serde_json::{json, Value};

use crate::state::State;

/// Past queries, newest first; clicking one reopens it in an editor tab.
pub(crate) fn history_list(st: &State) -> Value {
    let items: Vec<Value> = st
        .history
        .iter()
        .rev()
        .map(|h| {
            let conn = st
                .connections
                .iter()
                .find(|c| c.id == h.connection)
                .map(|c| c.name.as_str())
                .unwrap_or(h.connection.as_str());
            json!({ "title": one_line(&h.sql), "description": conn })
        })
        .collect();
    json!({
        "type": "list", "id": "history-list", "items": items,
        "empty-label": "No queries yet — run one to see it here."
    })
}

/// Collapse a (possibly multi-line) query to a single line for list display.
fn one_line(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}
