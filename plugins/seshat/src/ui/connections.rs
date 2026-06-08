//! The connections list and the connections-manager view.

use serde_json::{json, Value};

use crate::state::{engine_badge, Connection, State};
use crate::ui::widgets::button;
use crate::{ICON_DATABASE, ICON_PENCIL, ICON_PLUS, ICON_TRASH};

/// The full connections-manager pane (shown in an editor tab with no connection).
pub(crate) fn connections_view(st: &State) -> Value {
    let saved = st.connections.len();
    json!({
        "type": "column", "gap": 0, "children": [
            { "type": "row", "padding": 12, "gap": 10, "align": "center", "children": [
                button("new-connection", "New connection", "Elevated", "Primary", Some(ICON_PLUS), true, false),
                { "type": "text", "muted": true,
                  "value": format!("{saved} saved connection{}", if saved == 1 { "" } else { "s" }) }
            ]},
            { "type": "separator" },
            { "type": "scroll", "id": "conn-scroll", "child": connections_list(st) }
        ]
    })
}

/// The saved-connections `list` node — shared by the manager view and the sidebar.
pub(crate) fn connections_list(st: &State) -> Value {
    let items: Vec<Value> = st
        .connections
        .iter()
        .map(|c| connection_item(c, st.active.as_deref() == Some(&c.id)))
        .collect();
    json!({
        "type": "list",
        "id": "connections-list",
        "items": items,
        "empty-label": "No saved connections yet — click \u{201c}New connection\u{201d} to add one."
    })
}

fn connection_item(c: &Connection, active: bool) -> Value {
    let (short, color) = engine_badge(c.engine);
    let badge = if active {
        json!({ "text": "active", "color": "green" })
    } else {
        json!({ "text": short, "color": color })
    };
    json!({
        "title": c.name,
        "description": c.summary(),
        "icon": ICON_DATABASE,
        "badge": badge,
        "actions": [
            { "icon": ICON_PENCIL, "tooltip": "Edit connection" },
            { "icon": ICON_TRASH, "tooltip": "Delete connection" }
        ]
    })
}
