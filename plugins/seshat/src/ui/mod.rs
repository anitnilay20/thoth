//! UiNode-DSL view builders. The connections manager, SQL editor, and
//! new-connection dialog live here; the sidebar view is in [`sidebar`].

pub(crate) mod sidebar;
pub(crate) use sidebar::build_sidebar;

use serde_json::{json, Value};

use crate::state::{engine_badge, engine_value, Connection, State};
use crate::{
    ICON_CARET_LEFT, ICON_DATABASE, ICON_PENCIL, ICON_PLAY, ICON_PLUG, ICON_PLUS, ICON_TRASH,
};

/// Root view: connections manager or editor, with the new-connection modal on top.
pub(crate) fn build_ui(st: &State) -> Value {
    let main = if st.active.is_some() {
        editor_view(st)
    } else {
        connections_view(st)
    };
    json!({ "type": "column", "gap": 0, "children": [ main, dialog(st) ] })
}

fn connections_view(st: &State) -> Value {
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

/// The saved-connections `list` node — shared by the main view and the sidebar.
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

fn editor_view(st: &State) -> Value {
    let active = st
        .active
        .as_deref()
        .and_then(|id| st.connections.iter().find(|c| c.id == id));
    let title = active
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "Query".into());
    let subtitle = active.map(|c| c.summary()).unwrap_or_default();

    json!({
        "type": "column", "gap": 0, "children": [
            { "type": "row", "padding": 8, "gap": 8, "align": "center", "children": [
                { "type": "icon-button", "id": "back-to-connections", "icon": ICON_CARET_LEFT,
                  "tooltip": "Back to connections", "button-size": "Medium" },
                { "type": "heading", "value": title, "panel": true },
                { "type": "text", "muted": true, "value": subtitle }
            ]},
            { "type": "separator" },
            { "type": "code-editor", "id": "sql", "value": st.sql },
            { "type": "row", "padding": 8, "gap": 8, "children": [
                button("run", "Run", "Elevated", "Primary", Some(ICON_PLAY), !st.loading, false)
            ]},
            { "type": "separator" },
            { "type": "scroll", "id": "results-scroll", "child": results(st) }
        ]
    })
}

fn results(st: &State) -> Value {
    if st.loading {
        return json!({ "type": "row", "padding": 16, "gap": 10, "align": "center", "children": [
            { "type": "spinner" },
            { "type": "text", "muted": true, "value": "Running query\u{2026}" }
        ]});
    }
    match &st.result {
        Some(Ok(rows)) => json!({ "type": "json-tree", "value": rows }),
        Some(Err(msg)) => json!({ "type": "row", "padding": 12, "children": [
            { "type": "colored", "color": "#f38ba8",
              "child": { "type": "text", "value": format!("Error: {msg}") } }
        ]}),
        None => json!({ "type": "row", "padding": 12, "children": [
            { "type": "text", "muted": true, "value": "Run a query to see results." }
        ]}),
    }
}

/// The new-connection modal. Shared by the main view and the sidebar (each runs
/// as its own wasm instance, so each carries its own copy).
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

    let editing = st.editing.is_some();
    let (title, connect_label) = if editing {
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

// ── primitives ────────────────────────────────────────────────────────────────

fn text_input(id: &str, label: &str, value: &str, grow: bool, placeholder: &str) -> Value {
    json!({
        "type": "text-input", "id": id, "label": label,
        "value": value, "placeholder": placeholder, "grow": grow
    })
}

#[allow(clippy::too_many_arguments)]
fn button(
    id: &str,
    label: &str,
    btype: &str,
    color: &str,
    icon: Option<&str>,
    enabled: bool,
    full_width: bool,
) -> Value {
    let mut props = json!({
        "label": label,
        "button-type": btype,
        "color": color,
        "enabled": enabled,
        "full-width": full_width
    });
    if let Some(icon) = icon {
        props["icon"] = json!(icon);
    }
    json!({ "type": "button", "id": id, "props": props })
}
