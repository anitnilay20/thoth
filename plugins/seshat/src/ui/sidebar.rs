//! The Seshat sidebar: a top-tabbed navigator (Connections / Schema / History)
//! with icon-only tab headers and a "+" action to add a connection.
//!
//! The sidebar runs as its own wasm instance and the host only re-renders it
//! after a sidebar event, so it carries its own copy of the new-connection modal.

use serde_json::{json, Value};

use crate::state::State;
use crate::ui::connections::connections_list;
use crate::ui::dialog::dialog;
use crate::ui::error::error_modal;
use crate::ui::history::history_list;
use crate::ui::schema::schema_panel;
use crate::ui::widgets::button;
use crate::{ICON_HISTORY, ICON_PLUGS_CONNECTED, ICON_PLUS, ICON_TERMINAL, ICON_TREE_STRUCTURE};

pub(crate) fn build_sidebar(st: &State) -> Value {
    json!({
        "type": "column", "gap": 0, "children": [
            { "type": "row", "padding": 8, "children": [
                button("new-query", "New query", "Elevated", "Primary",
                       Some(ICON_TERMINAL), st.active.is_some(), true)
            ]},
            {
                "type": "tabs",
                "id": "sidebar-tabs",
                "header": ["Connections", "Schema", "History"],
                "icons": [ICON_PLUGS_CONNECTED, ICON_TREE_STRUCTURE, ICON_HISTORY],
                // Contrast the strip against the panel-colored sidebar.
                "bg-color": "bg-sunken",
                "actions": [
                    { "id": "new-connection", "icon": ICON_PLUS, "tooltip": "New connection" }
                ],
                "children": [
                    section("CONNECTIONS", connections_list(st)),
                    section("SCHEMA", schema_panel(st)),
                    section("HISTORY", history_list(st))
                ]
            },
            dialog(st),
            error_modal(st)
        ]
    })
}

/// Wrap a tab's body with a sidebar panel header and a divider.
fn section(title: &str, body: Value) -> Value {
    json!({ "type": "column", "gap": 0, "children": [
        { "type": "row", "padding": 6, "children": [
            { "type": "heading", "value": title, "panel": true }
        ]},
        { "type": "separator" },
        body
    ]})
}
