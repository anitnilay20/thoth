//! `RenderNode`-DSL view builders, split by section: the [`sidebar`] navigator,
//! the [`connections`] manager, the SQL [`editor`], the [`schema`] tree, the
//! [`history`] list, the new-connection [`dialog`], and shared [`widgets`].

pub(crate) mod connections;
pub(crate) mod dialog;
pub(crate) mod editor;
pub(crate) mod error;
pub(crate) mod history;
pub(crate) mod results;
pub(crate) mod schema;
pub(crate) mod sidebar;
pub(crate) mod structure;
pub(crate) mod widgets;

use thoth_plugin_sdk::components::Column;
use thoth_plugin_sdk::render_node::RenderNode;

use crate::state::{State, View};

pub(crate) use sidebar::build_sidebar;

/// Root of an editor tab: the connections manager (no active connection), a
/// table's structure view, or the SQL editor, with the modals layered on top.
pub(crate) fn build_ui(st: &State) -> RenderNode {
    let main = match &st.view {
        View::Structure { .. } => structure::structure_view(st),
        View::Editor if st.active.is_some() => editor::editor_view(st),
        View::Editor => connections::connections_view(st),
    };
    RenderNode::Column(
        Column::builder()
            .gap(0.0)
            .children(vec![main, dialog::dialog(st, ""), error::error_modal(st)])
            .build(),
    )
}
