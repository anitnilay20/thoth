//! The Seshat sidebar view: a compact saved-connections list with a "+" to add.
//!
//! The sidebar runs as its own wasm instance, and the host only re-renders the
//! sidebar after a sidebar event — so it carries its own copy of the
//! new-connection modal (a modal in the main pane wouldn't show from here).

use serde_json::{json, Value};

use crate::state::State;
use crate::ui::{connections_list, dialog};
use crate::ICON_PLUS;

pub(crate) fn build_sidebar(st: &State) -> Value {
    // The host already wraps sidebar content in an 8px margin and built-in
    // section headers sit flush against it — so add no extra padding here, or the
    // "CONNECTIONS" header sits lower than every other section's header.
    json!({
        "type": "column", "gap": 0, "children": [
            { "type": "row", "gap": 8, "align": "fill", "children": [
                { "type": "heading", "value": "CONNECTIONS", "panel": true },
                { "type": "spacer" },
                { "type": "icon-button", "id": "new-connection", "icon": ICON_PLUS,
                  "tooltip": "New connection", "frame": false, "button-size": "Small" }
            ]},
            { "type": "scroll", "id": "sidebar-scroll", "child": connections_list(st) },
            dialog(st)
        ]
    })
}
