//! The Seshat sidebar: a top-tabbed navigator (Connections / Schema / History)
//! with icon-only tab headers and a "+" action to add a connection.
//!
//! The sidebar runs as its own wasm instance and the host only re-renders it
//! after a sidebar event, so it carries its own copy of the new-connection modal.

use serde_json::{json, Value};

use crate::state::State;
use crate::ui::{connections_list, dialog, history_list, schema_panel};
use crate::{ICON_HISTORY, ICON_PLUGS_CONNECTED, ICON_PLUS, ICON_TREE_STRUCTURE};

pub(crate) fn build_sidebar(st: &State) -> Value {
    json!({
        "type": "column", "gap": 0, "children": [
            {
                "type": "tabs",
                "id": "sidebar-tabs",
                "header": ["Connections", "Schema", "History"],
                "icons": [ICON_PLUGS_CONNECTED, ICON_TREE_STRUCTURE, ICON_HISTORY],
                "actions": [
                    { "id": "new-connection", "icon": ICON_PLUS, "tooltip": "New connection" }
                ],
                "children": [
                    connections_list(st),
                    schema_panel(st),
                    history_list(st)
                ]
            },
            dialog(st)
        ]
    })
}
