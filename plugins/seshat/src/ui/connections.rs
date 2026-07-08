//! The connections list and the connections-manager view.

use thoth_plugin_sdk::components::{
    Align, Column, List, ListItem, ListItemAction, ListItemBadge, ListItemPrefix, Row, Scroll,
    Separator, Typography, TypographyVariant,
};
use thoth_plugin_sdk::render_node::RenderNode;

use crate::state::{engine_badge, Connection, State};
use crate::ui::widgets::{button, muted};
use crate::{ICON_DATABASE, ICON_PENCIL, ICON_PLUS, ICON_TRASH};

/// Environment colour tokens in display order, with their group labels. A
/// connection's `color` token places it in a group (matching the design's
/// prod/staging/dev grouping); anything else falls into "Ungrouped".
const ENV_GROUPS: [(&str, &str); 5] = [
    ("error", "Production"),
    ("warning", "Staging"),
    ("success", "Development"),
    ("accent", "Blue"),
    ("secondary", "Purple"),
];

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
                // connections_list owns its scroll, so it isn't wrapped again here.
                connections_list(st),
            ])
            .build(),
    )
}

/// The saved-connections `list` node — shared by the manager view and the
/// sidebar. Connections are grouped by their environment colour (like the
/// design handoff); each group is its own `List` (id `conn-grp-<n>`) so clicks
/// resolve back to `st.connections` via [`connection_groups`].
pub(crate) fn connections_list(st: &State) -> RenderNode {
    if st.connections.is_empty() {
        return RenderNode::List(
            List::builder()
                .id("conn-grp-0")
                .items(Vec::<ListItem>::new())
                .empty_label(
                    "No saved connections yet — click \u{201c}New connection\u{201d} to add one.",
                )
                .build(),
        );
    }
    let groups = connection_groups(st);
    // Show env headers unless everything is a single ungrouped bucket.
    let show_headers = groups.len() > 1 || groups.first().is_some_and(|g| g.1.is_some());

    let mut children: Vec<RenderNode> = Vec::new();
    for (gi, (label, color, members)) in groups.iter().enumerate() {
        if show_headers {
            children.push(group_header(label, color.as_deref(), members.len()));
        }
        let items: Vec<ListItem> = members
            .iter()
            .map(|&i| {
                let c = &st.connections[i];
                connection_item(
                    c,
                    st.active.as_deref() == Some(&c.id),
                    st.failed.as_deref() == Some(&c.id),
                )
            })
            .collect();
        children.push(RenderNode::List(
            List::builder()
                .id(format!("conn-grp-{gi}"))
                .items(items)
                // Size each group to its rows so groups stack tightly; the outer
                // scroll (below) handles overflow — otherwise each list fills the
                // pane and shoves the next group to the bottom.
                .shrink_to_fit(true)
                .build(),
        ));
    }
    // One scroll around the stacked groups (shared by the sidebar + manager view).
    RenderNode::Scroll(
        Scroll::builder()
            .id("connections-scroll")
            .child(RenderNode::Column(
                Column::builder().gap(0.0).children(children).build(),
            ))
            .build(),
    )
}

/// Connections grouped by environment colour, in display order. Returns
/// `(label, colour token, global indices into st.connections)` per non-empty
/// group. Shared by the renderer and the click handler so a group-local index
/// maps back to the right connection.
pub(crate) fn connection_groups(st: &State) -> Vec<(String, Option<String>, Vec<usize>)> {
    let mut groups: Vec<(String, Option<String>, Vec<usize>)> = Vec::new();
    for (token, label) in ENV_GROUPS {
        let members: Vec<usize> = st
            .connections
            .iter()
            .enumerate()
            .filter(|(_, c)| c.color.as_deref() == Some(token))
            .map(|(i, _)| i)
            .collect();
        if !members.is_empty() {
            groups.push((label.to_string(), Some(token.to_string()), members));
        }
    }
    // No colour (or an unrecognised one) → "Ungrouped", last.
    let known: Vec<&str> = ENV_GROUPS.iter().map(|(t, _)| *t).collect();
    let rest: Vec<usize> = st
        .connections
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            c.color
                .as_deref()
                .map(|t| !known.contains(&t))
                .unwrap_or(true)
        })
        .map(|(i, _)| i)
        .collect();
    if !rest.is_empty() {
        groups.push(("Ungrouped".to_string(), None, rest));
    }
    groups
}

/// A group header: the environment label (in its colour) + the member count.
fn group_header(label: &str, color: Option<&str>, count: usize) -> RenderNode {
    RenderNode::Row(
        Row::builder()
            .padding(6.0)
            .gap(8.0)
            .align(Align::Center)
            .children(vec![
                RenderNode::Text(
                    Typography::builder()
                        .text(label.to_uppercase())
                        .variant(TypographyVariant::GroupLabel)
                        .color(color.unwrap_or("muted"))
                        .build(),
                ),
                muted(&count.to_string()),
            ])
            .build(),
    )
}

fn connection_item(c: &Connection, active: bool, failed: bool) -> ListItem {
    let (short, badge_color) = engine_badge(c.engine);
    // The leading database icon doubles as a status dot: green when connected,
    // red on a failed connect, else the connection's environment colour (or muted).
    let status_color = if failed {
        "error"
    } else if active {
        "success"
    } else {
        c.color.as_deref().unwrap_or("muted")
    };
    // Badge colours are semantic tokens, resolved by the SDK against the theme.
    let badge = if failed {
        ListItemBadge::builder().text("error").color("red").build()
    } else if active {
        ListItemBadge::builder()
            .text("active")
            .color("green")
            .build()
    } else {
        ListItemBadge::builder()
            .text(short)
            .color(badge_color)
            .build()
    };
    ListItem::builder()
        .title(c.name.clone())
        .description(c.summary())
        .prefix(ListItemPrefix::Icon {
            glyph: ICON_DATABASE.to_string(),
            color: Some(status_color.to_string()),
        })
        .badge(badge)
        .selected(active)
        // Environment colour → left accent stripe.
        .maybe_accent(c.color.clone())
        .actions(vec![
            ListItemAction::builder()
                .icon(ICON_PENCIL)
                .tooltip("Edit connection")
                .build(),
            ListItemAction::builder()
                .icon(ICON_TRASH)
                .tooltip("Delete connection")
                .build(),
        ])
        .build()
}
