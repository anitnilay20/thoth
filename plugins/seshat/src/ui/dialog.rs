//! The new/edit-connection modal — a two-step wizard: pick an engine, then
//! enter credentials (matching the design handoff's `NewConnectionDialog`).

use thoth_plugin_sdk::components::{
    Align, Checkbox, Colored, Column, Input, List, ListItem, ListItemBadge, Modal, Row, Select,
    SelectOption, Separator, Size, Spacer, Split, Typography,
};
use thoth_plugin_sdk::render_node::RenderNode;

use crate::state::{engine_badge, engine_label, State, SUPPORTED_ENGINES};
use crate::ui::widgets::{button, text_input};
use crate::ICON_PLUG;

/// The new/edit-connection modal. Shared by the manager view and the sidebar.
///
/// `prefix` scopes the egui widget ids per rendering surface so the modal can
/// coexist in the same egui frame (sidebar panel + editor tab) without id
/// collisions. The event router strips this prefix before matching. Pass `""`
/// for the editor tab and `"sb-"` for the sidebar.
pub(crate) fn dialog(st: &State, prefix: &str) -> RenderNode {
    let editing = st.editing.is_some();
    let (title, connect_label) = if editing {
        ("Edit connection", "Save")
    } else {
        ("New connection", "Connect")
    };
    let subtitle = if st.dialog_form_step {
        format!("{} · enter credentials", engine_label(st.form.engine))
    } else {
        "Pick a database engine".to_string()
    };

    let body = if st.dialog_form_step {
        form_step(st, prefix, connect_label)
    } else {
        engine_step(prefix)
    };

    RenderNode::Modal(Box::new(
        Modal::builder()
            .id(format!("{prefix}new-connection-dialog"))
            .title(title)
            .subtitle(subtitle)
            .open(st.dialog_open)
            .close_id(format!("{prefix}dialog-close"))
            .width(640.0)
            .children(vec![body])
            .build(),
    ))
}

/// Step 0 — a framed list of engine cards (colour-coded badge + label + kind).
fn engine_step(prefix: &str) -> RenderNode {
    let items: Vec<ListItem> = SUPPORTED_ENGINES
        .iter()
        .map(|&e| {
            let (short, color) = engine_badge(e);
            let kind = if e == crate::db::Engine::Elasticsearch {
                "Search"
            } else {
                "SQL"
            };
            ListItem::builder()
                .title(engine_label(e))
                .description(kind)
                .badge(ListItemBadge::builder().text(short).color(color).build())
                .build()
        })
        .collect();

    RenderNode::Column(
        Column::builder()
            .gap(12.0)
            .children(vec![
                RenderNode::List(
                    List::builder()
                        .id(format!("{prefix}engine-list"))
                        .items(items)
                        .shrink_to_fit(true)
                        .framed(true)
                        .build(),
                ),
                footer(prefix, false, "Connect"),
            ])
            .build(),
    )
}

/// Step 1 — the credentials form matching the design's field layout.
fn form_step(st: &State, prefix: &str, connect_label: &str) -> RenderNode {
    let id = |name: &str| format!("{prefix}{name}");
    let db_placeholder = crate::db::adapter(st.form.engine)
        .connection_defaults()
        .database_placeholder;
    let mut fields: Vec<RenderNode> = vec![
        text_input(
            &id("f-name"),
            "Connection name",
            &st.form.name,
            true,
            "my-database",
        ),
        // Host (2 parts) + Port (1 part).
        RenderNode::Split(
            Split::builder()
                .gap(12.0)
                .widths(vec![2.0, 1.0])
                .children(vec![
                    text_input(&id("f-host"), "Host", &st.form.host, true, "localhost"),
                    text_input(&id("f-port"), "Port", &st.form.port, true, "5432"),
                ])
                .build(),
        ),
        text_input(
            &id("f-database"),
            "Database",
            &st.form.database,
            true,
            db_placeholder,
        ),
    ];

    // Authentication section. Elasticsearch supports three modes (None / Password
    // / API key) chosen via a selector; SQL engines are always password-based, so
    // they just show the User + Password fields.
    fields.extend(auth_fields(st, prefix));

    fields.push(RenderNode::Checkbox(
        Checkbox::builder()
            .id(id("f-tls"))
            .label("Require TLS")
            .checked(st.form.tls)
            .build(),
    ));
    fields.push(
        // Environment colour: a semantic token shown as the connection's accent
        // + status-dot tint, so prod vs local reads at a glance.
        RenderNode::Select(
            Select::builder()
                .id(id("f-color"))
                .value(st.form.color.clone())
                .prefix_label("Colour: ")
                .options(vec![
                    SelectOption::builder().value("").label("None").build(),
                    SelectOption::builder()
                        .value("error")
                        .label("Red · prod")
                        .build(),
                    SelectOption::builder()
                        .value("warning")
                        .label("Amber · staging")
                        .build(),
                    SelectOption::builder()
                        .value("success")
                        .label("Green · dev")
                        .build(),
                    SelectOption::builder()
                        .value("accent")
                        .label("Blue")
                        .build(),
                    SelectOption::builder()
                        .value("secondary")
                        .label("Purple")
                        .build(),
                ])
                .size(Size::Small)
                .build(),
        ),
    );

    if let Some(status) = &st.test_status {
        let (color, text) = match status {
            Ok(msg) => ("success", msg.clone()),
            Err(msg) => ("error", msg.clone()),
        };
        fields.push(RenderNode::Colored(
            Colored::builder()
                .color(color)
                .child(RenderNode::Text(Typography::builder().text(text).build()))
                .build(),
        ));
    }

    fields.push(footer(prefix, true, connect_label));

    RenderNode::Column(Column::builder().gap(10.0).children(fields).build())
}

/// The authentication fields for the credentials step.
///
/// SQL engines (Postgres/MySQL) are always username+password, so they show the
/// plain User + Password split with no selector. Elasticsearch adds a mode
/// selector (None / Password / API key) and swaps the credential inputs to match
/// the chosen mode.
fn auth_fields(st: &State, prefix: &str) -> Vec<RenderNode> {
    use crate::db::{AuthMode, Engine};
    let id = |name: &str| format!("{prefix}{name}");

    // User + Password split, reused by SQL engines and ES password mode.
    let user_password = || {
        RenderNode::Split(
            Split::builder()
                .gap(12.0)
                .widths(vec![1.0, 1.0])
                .children(vec![
                    text_input(&id("f-user"), "User", &st.form.user, true, ""),
                    RenderNode::Input(
                        Input::builder()
                            .id(id("f-password"))
                            .label("Password")
                            .value(st.form.password.clone())
                            .password(true)
                            .grow(true)
                            .build(),
                    ),
                ])
                .build(),
        )
    };

    // Non-Elasticsearch engines: no auth selector, just credentials.
    if st.form.engine != Engine::Elasticsearch {
        return vec![user_password()];
    }

    // Elasticsearch: a mode selector, then mode-specific inputs.
    let mode_value = match st.form.auth {
        AuthMode::None => "none",
        AuthMode::Password => "password",
        AuthMode::ApiKey => "apikey",
    };
    let selector = RenderNode::Select(
        Select::builder()
            .id(id("f-auth"))
            .value(mode_value.to_string())
            .prefix_label("Auth: ")
            .options(vec![
                SelectOption::builder()
                    .value("none")
                    .label("No authentication")
                    .build(),
                SelectOption::builder()
                    .value("password")
                    .label("Username & password")
                    .build(),
                SelectOption::builder()
                    .value("apikey")
                    .label("API key")
                    .build(),
            ])
            .size(Size::Small)
            .build(),
    );

    let mut out = vec![selector];
    match st.form.auth {
        AuthMode::None => {}
        AuthMode::Password => out.push(user_password()),
        AuthMode::ApiKey => out.push(RenderNode::Input(
            Input::builder()
                .id(id("f-apikey"))
                .label("API key (encoded)")
                .value(st.form.password.clone())
                .password(true)
                .grow(true)
                .build(),
        )),
    }
    out
}

/// The dialog footer: Back + Test on the form step, Cancel + Connect always
/// (Connect enabled only on the form step), pushed apart by a grow spacer.
fn footer(prefix: &str, form_step: bool, connect_label: &str) -> RenderNode {
    let id = |name: &str| format!("{prefix}{name}");
    let mut row: Vec<RenderNode> = Vec::new();
    if form_step {
        row.push(button(
            &id("dialog-back"),
            "Back",
            "Text",
            "Default",
            None,
            true,
            false,
        ));
        row.push(button(
            &id("dialog-test"),
            "Test connection",
            "Text",
            "Default",
            Some(ICON_PLUG),
            true,
            false,
        ));
    }
    row.push(RenderNode::Spacer(Spacer::builder().size(0.0).build()));
    row.push(button(
        &id("dialog-cancel"),
        "Cancel",
        "Text",
        "Default",
        None,
        true,
        false,
    ));
    row.push(button(
        &id("dialog-connect"),
        connect_label,
        "Elevated",
        "Primary",
        Some(ICON_PLUG),
        form_step,
        false,
    ));

    RenderNode::Column(
        Column::builder()
            .gap(10.0)
            .children(vec![
                RenderNode::Separator(Separator::plain()),
                RenderNode::Row(
                    Row::builder()
                        .gap(8.0)
                        .align(Align::Fill)
                        .children(row)
                        .build(),
                ),
            ])
            .build(),
    )
}
