//! The new/edit-connection modal dialog.

use thoth_plugin_sdk::components::{
    Align, Checkbox, Colored, Column, Input, Modal, Row, Select, SelectOption, Typography,
};
use thoth_plugin_sdk::render_node::RenderNode;

use crate::state::{engine_value, State};
use crate::ui::widgets::{button, text_input};
use crate::ICON_PLUG;

/// The new/edit-connection modal. Shared by the manager view and the sidebar.
pub(crate) fn dialog(st: &State) -> RenderNode {
    let mut form_children: Vec<RenderNode> = vec![
        text_input("f-name", "Name", &st.form.name, false, "my-database"),
        RenderNode::Select(
            Select::builder()
                .id("f-engine")
                .value(engine_value(st.form.engine))
                .options(vec![
                    SelectOption::builder().value("postgres").label("PostgreSQL").build(),
                    SelectOption::builder().value("mysql").label("MySQL").build(),
                ])
                .build(),
        ),
        RenderNode::Row(
            Row::builder()
                .gap(8.0)
                .children(vec![
                    text_input("f-host", "Host", &st.form.host, true, "localhost"),
                    text_input("f-port", "Port", &st.form.port, false, "5432"),
                ])
                .build(),
        ),
        text_input("f-database", "Database", &st.form.database, false, "postgres"),
        text_input("f-user", "User", &st.form.user, false, ""),
        RenderNode::Input(
            Input::builder()
                .id("f-password")
                .label("Password")
                .value(st.form.password.clone())
                .password(true)
                .build(),
        ),
        RenderNode::Checkbox(
            Checkbox::builder()
                .id("f-tls")
                .label("Require TLS")
                .checked(st.form.tls)
                .build(),
        ),
    ];

    if let Some(status) = &st.test_status {
        let (color, text) = match status {
            Ok(msg) => ("#a6e3a1", msg.clone()),
            Err(msg) => ("#f38ba8", msg.clone()),
        };
        form_children.push(RenderNode::Colored(
            Colored::builder()
                .color(color)
                .child(RenderNode::Text(Typography::builder().text(text).build()))
                .build(),
        ));
    }

    let (title, connect_label) = if st.editing.is_some() {
        ("Edit connection", "Save")
    } else {
        ("New connection", "Connect")
    };

    form_children.push(RenderNode::Row(
        Row::builder()
            .gap(8.0)
            .align(Align::Center)
            .children(vec![
                button("dialog-test", "Test connection", "Text", "Default", Some(ICON_PLUG), true, false),
                button("dialog-cancel", "Cancel", "Text", "Default", None, true, false),
                button("dialog-connect", connect_label, "Elevated", "Primary", Some(ICON_PLUG), true, false),
            ])
            .build(),
    ));

    RenderNode::Modal(Box::new(
        Modal::builder()
            .id("new-connection-dialog")
            .title(title)
            .open(st.dialog_open)
            .close_id("dialog-close")
            .width_pct(0.5)
            .children(vec![RenderNode::Column(
                Column::builder().gap(10.0).children(form_children).build(),
            )])
            .build(),
    ))
}
