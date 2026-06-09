//! Event handling: translate widget events into state transitions, and fold
//! async `query-result` events back into the UI state.

use serde_json::Value;

use crate::bindings::exports::thoth::plugin::ui_component::UiEvent;
use crate::bindings::thoth::plugin::{secure_storage, ui_tabs};
use crate::db::{self, ColumnInfo, TableInfo};
use crate::state::{
    default_port, engine_from_value, make_id, pw_key, record_history, save_connections, submit,
    Connection, Form, Kind, Request, SchemaNode, State, TableNode,
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

    // History list: click an entry to reopen it in a fresh editor tab.
    if event.widget_id == "history-list" {
        if event.kind == "click" {
            if let Ok(i) = event.value.parse::<usize>() {
                open_history_entry(st, i);
            }
        }
        return;
    }

    // List events: a row click opens, an action (trash) deletes.
    if event.widget_id == "connections-list" {
        match event.kind.as_str() {
            "click" => {
                if let Ok(i) = event.value.parse::<usize>() {
                    if let Some(conn) = st.connections.get(i).cloned() {
                        // Activate for the sidebar Schema tab AND open an editor tab,
                        // handing it the password we just loaded (no second prompt).
                        activate_connection(st, &conn);
                        let pw = active_password(st, &conn.id);
                        open_tab(&conn.name, &conn.id, pw, None);
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
        "error-close" => st.error = None,
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
            st.schemas.clear();
            st.schema_loaded = false;
            st.schema_error = None;
        }
        // Schema-tree data-rows. Schema: any interaction toggles it. Table: the
        // caret ("toggle") expands columns, a body "click" opens a SELECT. Column
        // (leaf, id "col:<i>:<j>:<k>"): clicking opens its table's SELECT.
        id if id.starts_with("sch:") => {
            if let Ok(i) = id[4..].parse::<usize>() {
                toggle_schema(st, i);
            }
        }
        id if id.starts_with("tbl:") => {
            if let Some((i, j)) = parse_pair(&id[4..]) {
                if event.kind == "toggle" {
                    toggle_table(st, i, j);
                } else {
                    use_table(st, i, j);
                }
            }
        }
        id if id.starts_with("col:") => {
            let idx: Vec<usize> = id[4..].split(':').filter_map(|s| s.parse().ok()).collect();
            if let [i, j, ..] = idx[..] {
                use_table(st, i, j);
            }
        }
        "sql" => st.sql = parse_str(&event.value),
        "run" if !st.loading => {
            st.loading = true;
            st.result = None;
            let sql = st.sql.clone();
            if let Some(id) = st.active.clone() {
                record_history(&id, &sql);
            }
            submit(&Request::Query { sql }, Kind::Query, st);
        }
        _ => {}
    }
}

/// Open an editor tab seeded with a connection (and optionally its password +
/// SQL). Passing the password lets the new instance skip a keychain read — and
/// therefore the macOS keychain prompt — for tabs opened during the session.
fn open_tab(name: &str, conn_id: &str, password: Option<&str>, sql: Option<&str>) {
    let mut state = serde_json::json!({ "connection": conn_id });
    if let Some(p) = password {
        state["password"] = Value::from(p);
    }
    if let Some(s) = sql {
        state["sql"] = Value::from(s);
    }
    ui_tabs::open_tab(name, Some(crate::ICON_TERMINAL), Some(&state.to_string()));
}

/// The in-memory password for the active connection, if it matches `conn_id`
/// (so we can hand it to a new tab instead of having that tab re-read the keychain).
fn active_password<'a>(st: &'a State, conn_id: &str) -> Option<&'a str> {
    if st.active.as_deref() == Some(conn_id) {
        st.active_profile.as_ref().map(|p| p.password.as_str())
    } else {
        None
    }
}

/// The password for `id`: from the session cache, else read once from the
/// keychain and cached (so repeat selects/edits don't re-prompt).
fn load_password(st: &mut State, id: &str) -> String {
    if let Some(p) = st.password_cache.get(id) {
        return p.clone();
    }
    let p = secure_storage::read(&pw_key(id))
        .ok()
        .flatten()
        .unwrap_or_default();
    st.password_cache.insert(id.to_string(), p.clone());
    p
}

/// Activate a connection in *this* instance: load its password into the session
/// profile and mark it active (so render_ui shows the editor).
fn activate_connection(st: &mut State, conn: &Connection) {
    let password = load_password(st, &conn.id);
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
    st.schemas.clear();
    st.schema_loaded = false;
    st.schema_error = None;
    st.failed = None;
    st.error = None;
    load_schemas(st);
}

// ── schema browser ────────────────────────────────────────────────────────────

fn parse_pair(s: &str) -> Option<(usize, usize)> {
    let (a, b) = s.split_once(':')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// Kick off the schema list for the active connection (once).
fn load_schemas(st: &mut State) {
    if st.schema_loaded {
        return;
    }
    st.schema_loaded = true;
    st.schema_error = None;
    submit(&Request::ListSchemas, Kind::Schemas, st);
}

fn toggle_schema(st: &mut State, i: usize) {
    let Some(sch) = st.schemas.get_mut(i) else {
        return;
    };
    sch.expanded = !sch.expanded;
    let need_load = sch.expanded && sch.tables.is_none();
    let schema = sch.name.clone();
    if need_load {
        submit(
            &Request::ListTables {
                schema: schema.clone(),
            },
            Kind::Tables { schema },
            st,
        );
    }
}

fn toggle_table(st: &mut State, i: usize, j: usize) {
    let Some(sch) = st.schemas.get_mut(i) else {
        return;
    };
    let schema = sch.name.clone();
    let Some(tbl) = sch.tables.as_mut().and_then(|t| t.get_mut(j)) else {
        return;
    };
    tbl.expanded = !tbl.expanded;
    let need_load = tbl.expanded && tbl.columns.is_none();
    let table = tbl.name.clone();
    if need_load {
        submit(
            &Request::ListColumns {
                schema: schema.clone(),
                table: table.clone(),
            },
            Kind::Columns { schema, table },
            st,
        );
    }
}

/// Open an editor tab for the chosen table, prefilled with a `SELECT *`.
fn use_table(st: &State, i: usize, j: usize) {
    let target = st.schemas.get(i).and_then(|sch| {
        sch.tables
            .as_ref()
            .and_then(|t| t.get(j))
            .map(|tbl| (sch.name.clone(), tbl.name.clone()))
    });
    let Some((schema, table)) = target else {
        return;
    };
    let Some(conn) = st
        .active
        .as_deref()
        .and_then(|id| st.connections.iter().find(|c| c.id == id))
    else {
        return;
    };
    let sql = format!("SELECT * FROM \"{schema}\".\"{table}\" LIMIT 100;");
    open_tab(
        &conn.name,
        &conn.id,
        active_password(st, &conn.id),
        Some(&sql),
    );
}

/// Reopen a history entry (shown newest-first) in a fresh editor tab.
fn open_history_entry(st: &State, display_index: usize) {
    let Some(entry) = st.history.iter().rev().nth(display_index) else {
        return;
    };
    let title = st
        .connections
        .iter()
        .find(|c| c.id == entry.connection)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| entry.connection.clone());
    open_tab(
        &title,
        &entry.connection,
        active_password(st, &entry.connection),
        Some(&entry.sql),
    );
}

/// Seed an editor-tab instance from its initial-state blob
/// (`{connection, password?, sql?}`). Uses a handed-in password when present to
/// avoid a keychain read, falling back to the keychain otherwise. Does NOT load
/// the schema — that lives in the sidebar — so opening a tab makes no connection
/// until the user runs a query.
pub(crate) fn activate_from_state(st: &mut State, state: &str) {
    let Ok(v) = serde_json::from_str::<Value>(state) else {
        return;
    };
    if let Some(conn) = v
        .get("connection")
        .and_then(|c| c.as_str())
        .and_then(|id| st.connections.iter().find(|c| c.id == id).cloned())
    {
        let password = v
            .get("password")
            .and_then(|p| p.as_str())
            .map(String::from)
            .unwrap_or_else(|| {
                secure_storage::read(&pw_key(&conn.id))
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            });
        st.active_profile = Some(db::Profile {
            host: conn.host.clone(),
            port: conn.port,
            database: conn.database.clone(),
            user: conn.user.clone(),
            password,
            tls: conn.tls,
        });
        st.active = Some(conn.id);
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
    let password = load_password(st, &conn.id);
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
    st.password_cache.remove(&conn.id);
    if st.active.as_deref() == Some(&conn.id) {
        st.active = None;
        st.active_profile = None;
    }
    save_connections(st);
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
    st.password_cache
        .insert(id.clone(), st.form.password.clone());
    // Hand the just-entered password to the new tab so it doesn't re-read the keychain.
    open_tab(&conn.name, &id, Some(&st.form.password), None);
    match st.connections.iter_mut().find(|c| c.id == id) {
        Some(existing) => *existing = conn,
        None => st.connections.push(conn),
    }
    save_connections(st);

    st.editing = None;
    st.dialog_open = false;
    st.test_status = None;
}

fn handle_query_result(st: &mut State, event: &UiEvent) {
    let Some(pos) = st.pending.iter().position(|(id, _)| id == &event.widget_id) else {
        return;
    };
    let kind = st.pending[pos].1.clone();
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
        match &kind {
            Kind::Test => st.test_status = Some(Ok("Waiting for host approval…".to_string())),
            Kind::Query => {
                st.loading = false;
                st.result = Some(Err("Waiting for host approval…".to_string()));
            }
            _ => {} // schema introspection: wait silently for approval
        }
        return; // keep the request pending for the re-run
    }

    st.pending.remove(pos);

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
        Kind::Schemas => match (ok, err) {
            (Some(v), _) => {
                let names: Vec<String> = decode_inner(v)
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                st.schemas = names
                    .into_iter()
                    .map(|name| SchemaNode {
                        name,
                        expanded: false,
                        tables: None,
                    })
                    .collect();
                st.schema_error = None;
                st.error = None;
                st.failed = None;
            }
            (None, m) => {
                // Listing schemas is our connection probe on select. On failure,
                // surface it in the error modal and mark the connection as errored
                // instead of leaving it active.
                let msg = m.unwrap_or_else(|| "failed to connect".into());
                st.failed = st.active.take();
                st.active_profile = None;
                st.schemas.clear();
                st.schema_error = Some(msg.clone());
                st.error = Some(msg);
            }
        },
        Kind::Tables { schema } => {
            let tables: Vec<TableInfo> = ok
                .map(|v| serde_json::from_value(decode_inner(v)).unwrap_or_default())
                .unwrap_or_default();
            if let Some(node) = st.schemas.iter_mut().find(|s| s.name == schema) {
                node.tables = Some(
                    tables
                        .into_iter()
                        .map(|t| TableNode {
                            name: t.name,
                            kind: t.kind,
                            expanded: false,
                            columns: None,
                        })
                        .collect(),
                );
            }
            if let Some(m) = err {
                st.schema_error = Some(m);
            }
        }
        Kind::Columns { schema, table } => {
            let cols: Vec<ColumnInfo> = ok
                .map(|v| serde_json::from_value(decode_inner(v)).unwrap_or_default())
                .unwrap_or_default();
            if let Some(node) = st
                .schemas
                .iter_mut()
                .find(|s| s.name == schema)
                .and_then(|s| s.tables.as_mut())
                .and_then(|ts| ts.iter_mut().find(|t| t.name == table))
            {
                node.columns = Some(cols);
            }
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
