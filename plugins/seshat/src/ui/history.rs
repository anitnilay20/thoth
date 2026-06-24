//! The query-history list.

use thoth_plugin_sdk::components::{List, ListItem};
use thoth_plugin_sdk::render_node::RenderNode;

use crate::state::State;

/// Past queries, newest first; clicking one reopens it in an editor tab.
pub(crate) fn history_list(st: &State) -> RenderNode {
    let items: Vec<ListItem> = st
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
            ListItem::builder()
                .title(one_line(&h.sql))
                .description(conn.to_string())
                .build()
        })
        .collect();
    RenderNode::List(
        List::builder()
            .id("history-list")
            .items(items)
            .empty_label("No queries yet — run one to see it here.")
            .build(),
    )
}

/// Collapse a (possibly multi-line) query to a single line for list display.
fn one_line(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}
