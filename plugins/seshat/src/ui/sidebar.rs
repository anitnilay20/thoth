//! The Seshat sidebar: a top-tabbed navigator (Connections / Schema / History)
//! with icon-only tab headers and a "+" action to add a connection.

use thoth_plugin_sdk::components::{
    Column, Row, Separator, TabAction, Tabs, Typography, TypographyVariant,
};
use thoth_plugin_sdk::render_node::RenderNode;

use crate::state::State;
use crate::ui::connections::connections_list;
use crate::ui::dialog::dialog;
use crate::ui::error::error_modal;
use crate::ui::history::history_list;
use crate::ui::schema::schema_panel;
use crate::ui::widgets::button;
use crate::{ICON_HISTORY, ICON_PLUGS_CONNECTED, ICON_PLUS, ICON_TERMINAL, ICON_TREE_STRUCTURE};

pub(crate) fn build_sidebar(st: &State) -> RenderNode {
    RenderNode::Column(
        Column::builder()
            .gap(0.0)
            .children(vec![
                RenderNode::Row(
                    Row::builder()
                        .padding(8.0)
                        .children(vec![button(
                            "new-query",
                            "New query",
                            "Elevated",
                            "Primary",
                            Some(ICON_TERMINAL),
                            st.active.is_some(),
                            true,
                        )])
                        .build(),
                ),
                RenderNode::Tabs(
                    Tabs::builder()
                        .id("sidebar-tabs")
                        .headers(vec![
                            "Connections".to_string(),
                            "Schema".to_string(),
                            "History".to_string(),
                        ])
                        .icons(vec![
                            ICON_PLUGS_CONNECTED.to_string(),
                            ICON_TREE_STRUCTURE.to_string(),
                            ICON_HISTORY.to_string(),
                        ])
                        .actions(vec![TabAction::builder()
                            .id("new-connection")
                            .icon(ICON_PLUS)
                            .tooltip("New connection")
                            .build()])
                        .children(vec![
                            section("CONNECTIONS", connections_list(st)),
                            section("SCHEMA", schema_panel(st)),
                            section("HISTORY", history_list(st)),
                        ])
                        .build(),
                ),
                dialog(st),
                error_modal(st),
            ])
            .build(),
    )
}

/// Wrap a tab's body with a sidebar panel header and a divider.
fn section(title: &str, body: RenderNode) -> RenderNode {
    RenderNode::Column(
        Column::builder()
            .gap(0.0)
            .children(vec![
                RenderNode::Row(
                    Row::builder()
                        .padding(6.0)
                        .children(vec![RenderNode::Text(
                            Typography::builder()
                                .text(title)
                                .variant(TypographyVariant::PanelHeader)
                                .build(),
                        )])
                        .build(),
                ),
                RenderNode::Separator(Separator::plain()),
                body,
            ])
            .build(),
    )
}
