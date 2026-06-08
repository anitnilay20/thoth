//! UiNode-DSL view builders, split by section: the [`sidebar`] navigator, the
//! [`connections`] manager, the SQL [`editor`], the [`schema`] tree, the
//! [`history`] list, the new-connection [`dialog`], and shared [`widgets`].

pub(crate) mod connections;
pub(crate) mod dialog;
pub(crate) mod editor;
pub(crate) mod error;
pub(crate) mod history;
pub(crate) mod schema;
pub(crate) mod sidebar;
pub(crate) mod widgets;

use serde_json::{json, Value};

use crate::state::State;

pub(crate) use sidebar::build_sidebar;

/// Root of an editor tab: the connections manager (no active connection) or the
/// SQL editor, with the new-connection modal layered on top.
pub(crate) fn build_ui(st: &State) -> Value {
    let main = if st.active.is_some() {
        editor::editor_view(st)
    } else {
        connections::connections_view(st)
    };
    json!({ "type": "column", "gap": 0, "children": [
        main,
        dialog::dialog(st),
        error::error_modal(st)
    ]})
}
