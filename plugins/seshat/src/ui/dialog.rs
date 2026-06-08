//! The new/edit-connection modal dialog.

use serde_json::{json, Value};

use crate::state::{engine_value, State};
use crate::ui::widgets::{button, text_input};
use crate::ICON_PLUG;

/// The new/edit-connection modal. Shared by the manager view and the sidebar
/// (each runs as its own wasm instance, so each carries its own copy).
pub(crate) fn dialog(st: &State) -> Value {
    let mut form_children = vec![
        text_input("f-name", "Name", &st.form.name, false, "my-database"),
        json!({
            "type": "select", "id": "f-engine", "label": "Engine",
            "value": engine_value(st.form.engine),
            "options": [
                { "value": "postgres", "label": "PostgreSQL" },
                { "value": "mysql", "label": "MySQL" }
            ]
        }),
        json!({ "type": "row", "gap": 8, "children": [
            text_input("f-host", "Host", &st.form.host, true, "localhost"),
            text_input("f-port", "Port", &st.form.port, false, "5432")
        ]}),
        text_input(
            "f-database",
            "Database",
            &st.form.database,
            false,
            "postgres",
        ),
        text_input("f-user", "User", &st.form.user, false, ""),
        json!({ "type": "password-input", "id": "f-password", "label": "Password",
                "value": st.form.password }),
        json!({ "type": "checkbox", "id": "f-tls", "label": "Require TLS", "checked": st.form.tls }),
    ];

    if let Some(status) = &st.test_status {
        let (color, text) = match status {
            Ok(msg) => ("#a6e3a1", msg.clone()),
            Err(msg) => ("#f38ba8", msg.clone()),
        };
        form_children.push(json!({
            "type": "colored", "color": color,
            "child": { "type": "text", "value": text }
        }));
    }

    let (title, connect_label) = if st.editing.is_some() {
        ("Edit connection", "Save")
    } else {
        ("New connection", "Connect")
    };

    form_children.push(json!({ "type": "row", "gap": 8, "align": "center", "children": [
        button("dialog-test", "Test connection", "Text", "Default", Some(ICON_PLUG), true, false),
        button("dialog-cancel", "Cancel", "Text", "Default", None, true, false),
        button("dialog-connect", connect_label, "Elevated", "Primary", Some(ICON_PLUG), true, false)
    ]}));

    json!({
        "type": "modal",
        "id": "new-connection-dialog",
        "title": title,
        "open": st.dialog_open,
        "close-id": "dialog-close",
        "width-pct": 0.5,
        "children": [ { "type": "column", "gap": 10, "children": form_children } ]
    })
}
