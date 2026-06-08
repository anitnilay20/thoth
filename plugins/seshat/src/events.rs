//! Event handling: translate widget events into state transitions, and fold
//! async `query-result` events back into the UI state.

use serde_json::Value;

use crate::bindings::exports::thoth::plugin::ui_component::UiEvent;
use crate::bindings::thoth::plugin::{secure_storage, ui_tabs};
use crate::db;
use crate::state::{
    default_port, engine_from_value, make_id, pw_key, save_state, submit, Connection, Form, Kind,
    Request, State,
};

/// Parse a widget value that may be a JSON-encoded string or a bare string.
fn parse_str(s: &str) -> String {
    serde_json::from_str::<String>(s).unwrap_or_else(|_| s.to_string())
}

pub(crate) fn apply_event(st: &mut State, event: &UiEvent) {
    // Async results arrive as a synthetic "query-result" event.
    if event.kind == "query-result" {
        handle_query_result(st, event);
        return;
    }

    // List events: a row click opens, an action (trash) deletes.
    if event.widget_id == "connections-list" {
        match event.kind.as_str() {
            "click" => {
                if let Ok(i) = event.value.parse::<usize>() {
                    if let Some(conn) = st.connections.get(i).cloned() {
                        open_editor_tab(&conn);
                    }
                }
            }
            "action" => {
                if let Ok(v) = serde_json::from_str::<Value>(&event.value) {
                    let item = v.get("item").and_then(|x| x.as_u64()).map(|i| i as usize);
                    let action = v.get("action").and_then(|x| x.as_u64()).unwrap_or(0);
                    if let Some(i) = item {
                        match action {
                            0 => edit_connection(st, i),   // pencil
                            _ => delete_connection(st, i), // trash
                        }
                    }
                }
            }
            _ => {}
        }
        return;
    }

    match event.widget_id.as_str() {
        // dialog form fields (also accept bare ids so the integration test can
        // populate a profile without going through the dialog)
        "f-name" => st.form.name = parse_str(&event.value),
        "f-engine" => {
            let e = engine_from_value(&parse_str(&event.value));
            st.form.engine = e;
            st.form.port = default_port(e).to_string();
        }
        "f-host" | "host" => st.form.host = parse_str(&event.value),
        "f-port" | "port" => st.form.port = parse_str(&event.value),
        "f-database" | "database" => st.form.database = parse_str(&event.value),
        "f-user" | "user" => st.form.user = parse_str(&event.value),
        "f-password" | "password" => st.form.password = parse_str(&event.value),
        "f-tls" | "tls" => st.form.tls = serde_json::from_str(&event.value).unwrap_or(false),

        "new-connection" => {
            st.editing = None;
            st.form = Form::default();
            st.test_status = None;
            st.dialog_open = true;
        }
        "dialog-close" | "dialog-cancel" => {
            st.dialog_open = false;
            st.editing = None;
            st.test_status = None;
        }
        "dialog-test" => {
            st.active_profile = Some(st.form.profile());
            st.test_status = Some(Ok("testing…".to_string()));
            submit(&Request::TestConnection, Kind::Test, st);
        }
        "dialog-connect" => connect_from_form(st),

        "back-to-connections" => {
            st.active = None;
            st.active_profile = None;
            st.result = None;
        }
        "sql" => st.sql = parse_str(&event.value),
        "run" if !st.loading => {
            st.loading = true;
            st.result = None;
            let sql = st.sql.clone();
            submit(&Request::Query { sql }, Kind::Query, st);
        }
        _ => {}
    }
}

/// Open a new editor tab seeded with `conn`. The host builds a fresh plugin
/// instance and calls `init_with_state`, which activates the connection.
fn open_editor_tab(conn: &Connection) {
    let state = serde_json::json!({ "connection": conn.id }).to_string();
    ui_tabs::open_tab(&conn.name, Some(crate::ICON_DATABASE), Some(&state));
}

/// Activate a connection in *this* instance: load its password from the keychain
/// into the session profile and mark it active (so render_ui shows the editor).
fn activate_connection(st: &mut State, conn: &Connection) {
    let password = secure_storage::read(&pw_key(&conn.id))
        .ok()
        .flatten()
        .unwrap_or_default();
    st.active_profile = Some(db::Profile {
        host: conn.host.clone(),
        port: conn.port,
        database: conn.database.clone(),
        user: conn.user.clone(),
        password,
        tls: conn.tls,
    });
    st.active = Some(conn.id.clone());
    st.result = None;
}

/// Seed this instance from a tab's initial-state blob (`{connection, sql?}`).
/// Called by `tab-host::init_with_state` when a Seshat editor tab opens.
pub(crate) fn activate_from_state(st: &mut State, state: &str) {
    let Ok(v) = serde_json::from_str::<Value>(state) else {
        return;
    };
    if let Some(conn) = v
        .get("connection")
        .and_then(|c| c.as_str())
        .and_then(|id| st.connections.iter().find(|c| c.id == id).cloned())
    {
        activate_connection(st, &conn);
    }
    if let Some(sql) = v.get("sql").and_then(|s| s.as_str()) {
        if !sql.is_empty() {
            st.sql = sql.to_string();
        }
    }
}

/// Open the dialog pre-filled with an existing connection, in edit mode.
fn edit_connection(st: &mut State, index: usize) {
    let Some(conn) = st.connections.get(index).cloned() else {
        return;
    };
    let password = secure_storage::read(&pw_key(&conn.id))
        .ok()
        .flatten()
        .unwrap_or_default();
    st.form = Form {
        name: conn.name.clone(),
        engine: conn.engine,
        host: conn.host.clone(),
        port: conn.port.to_string(),
        database: conn.database.clone(),
        user: conn.user.clone(),
        password,
        tls: conn.tls,
    };
    st.editing = Some(conn.id);
    st.test_status = None;
    st.dialog_open = true;
}

fn delete_connection(st: &mut State, index: usize) {
    if index >= st.connections.len() {
        return;
    }
    let conn = st.connections.remove(index);
    let _ = secure_storage::delete(&pw_key(&conn.id));
    if st.active.as_deref() == Some(&conn.id) {
        st.active = None;
        st.active_profile = None;
    }
    save_state(st);
}

/// Save the dialog form — updating the connection being edited, or creating a
/// new one — store its password in the keychain, and activate it.
fn connect_from_form(st: &mut State) {
    let name = if st.form.name.trim().is_empty() {
        st.form.host.clone()
    } else {
        st.form.name.trim().to_string()
    };
    let profile = st.form.profile();

    // Reuse the existing id when editing; otherwise mint a fresh slug.
    let id = st
        .editing
        .clone()
        .unwrap_or_else(|| make_id(&name, &st.connections));
    let conn = Connection {
        id: id.clone(),
        name,
        engine: st.form.engine,
        host: profile.host.clone(),
        port: profile.port,
        database: profile.database.clone(),
        user: profile.user.clone(),
        tls: profile.tls,
    };
    let _ = secure_storage::write(&pw_key(&id), &st.form.password);
    open_editor_tab(&conn);
    match st.connections.iter_mut().find(|c| c.id == id) {
        Some(existing) => *existing = conn,
        None => st.connections.push(conn),
    }
    save_state(st);

    st.editing = None;
    st.dialog_open = false;
    st.test_status = None;
}

fn handle_query_result(st: &mut State, event: &UiEvent) {
    let Some((req_id, kind)) = st.pending.clone() else {
        return;
    };
    if req_id != event.widget_id {
        return;
    }
    let parsed: Value = serde_json::from_str(&event.value).unwrap_or_default();
    let ok = parsed.get("ok");
    let err = parsed
        .get("err")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .map(String::from);

    // Host-gated connect: the request stays pending so the consent-approved
    // re-run (delivered under the same request id) still matches. Just surface a
    // "waiting" note rather than a hard error.
    if ok.is_none() && err.as_deref().is_some_and(|m| m.contains("consent")) {
        match kind {
            Kind::Test => st.test_status = Some(Ok("Waiting for host approval…".to_string())),
            Kind::Query => {
                st.loading = false;
                st.result = Some(Err("Waiting for host approval…".to_string()));
            }
        }
        return; // keep st.pending set for the re-run
    }

    st.pending = None;

    match kind {
        Kind::Test => {
            st.test_status = if let Some(v) = ok {
                Ok(format!("Connected · {}", short_version(v)))
            } else {
                Err(err.unwrap_or_else(|| "test failed".into()))
            }
            .into();
        }
        Kind::Query => {
            st.loading = false;
            st.result = Some(match (ok, err) {
                (Some(v), _) => Ok(decode_inner(v)),
                (None, Some(m)) => Err(m),
                _ => Err("query failed".into()),
            });
        }
    }
}

/// The host wraps `query()`'s String return as `{"ok": "<json-string>"}`; unwrap
/// the inner JSON so the UI sees the real value (object/array), not a string.
fn decode_inner(ok: &Value) -> Value {
    ok.as_str()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| ok.clone())
}

fn short_version(ok: &Value) -> String {
    let s = decode_inner(ok);
    let text = s.as_str().unwrap_or("ok");
    text.split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ")
}
