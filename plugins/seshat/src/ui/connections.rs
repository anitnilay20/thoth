//! The connections list and the connections-manager view.

use thoth_plugin_sdk::components::{
    Column, List, ListItem, ListItemAction, ListItemBadge, Row, Scroll, Separator,
};
use thoth_plugin_sdk::render_node::RenderNode;

use crate::state::{engine_badge, Connection, State};
use crate::ui::widgets::{button, muted};
use crate::{ICON_DATABASE, ICON_PENCIL, ICON_PLUS, ICON_TRASH};

/// The full connections-manager pane (shown in an editor tab with no connection).
pub(crate) fn connections_view(st: &State) -> RenderNode {
    let saved = st.connections.len();
    RenderNode::Column(
        Column::builder()
            .gap(0.0)
            .children(vec![
                RenderNode::Row(
                    Row::builder()
                        .padding(12.0)
                        .gap(10.0)
                        .align(thoth_plugin_sdk::components::Align::Center)
                        .children(vec![
                            button(
                                "new-connection",
                                "New connection",
                                "Elevated",
                                "Primary",
                                Some(ICON_PLUS),
                                true,
                                false,
                            ),
                            muted(&format!(
                                "{saved} saved connection{}",
                                if saved == 1 { "" } else { "s" }
                            )),
                        ])
                        .build(),
                ),
                RenderNode::Separator(Separator::plain()),
                RenderNode::Scroll(Scroll::builder().child(connections_list(st)).build()),
            ])
            .build(),
    )
}

/// The saved-connections `list` node — shared by the manager view and the sidebar.
pub(crate) fn connections_list(st: &State) -> RenderNode {
    let items: Vec<ListItem> = st
        .connections
        .iter()
        .map(|c| {
            let active = st.active.as_deref() == Some(&c.id);
            let failed = st.failed.as_deref() == Some(&c.id);
            connection_item(c, active, failed)
        })
        .collect();
    RenderNode::List(
        List::builder()
            .id("connections-list")
            .items(items)
            .empty_label("No saved connections yet — click \u{201c}New connection\u{201d} to add one.")
            .build(),
    )
}

fn connection_item(c: &Connection, active: bool, failed: bool) -> ListItem {
    let (short, color) = engine_badge(c.engine);
    // Badge colours are semantic tokens, resolved by the SDK against the theme.
    let badge = if failed {
        ListItemBadge::builder().text("error").color("red").build()
    } else if active {
        ListItemBadge::builder().text("active").color("green").build()
    } else {
        ListItemBadge::builder().text(short).color(color).build()
    };
    ListItem::builder()
        .title(c.name.clone())
        .description(c.summary())
        .icon(ICON_DATABASE)
        .badge(badge)
        .actions(vec![
            ListItemAction::builder().icon(ICON_PENCIL).tooltip("Edit connection").build(),
            ListItemAction::builder().icon(ICON_TRASH).tooltip("Delete connection").build(),
        ])
        .build()
}
