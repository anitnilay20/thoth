//! Event handling: translate widget events into state transitions, and fold
//! async `query-result` events back into the UI state.

use serde_json::Value;

use crate::bindings::exports::thoth::plugin::ui_component::UiEvent;
use crate::bindings::thoth::plugin::datasets::{self, Dataset, DatasetColumn};
use crate::bindings::thoth::plugin::signals::{self, Status as SignalStatus};
use crate::bindings::thoth::plugin::{file_dialog, secure_storage, ui_tabs};
use crate::db::{self, ColumnInfo, TableInfo};
use crate::sql;
use crate::state::{
    engine_from_value, make_id, pw_key, record_history, save_connections, submit, Connection,
    DatabaseNode, Form, Kind, Request, ResultsTab, SchemaNode, State, TableNode,
};

/// Parse a widget value that may be a JSON-encoded string or a bare string.
fn parse_str(s: &str) -> String {
    serde_json::from_str::<String>(s).unwrap_or_else(|_| s.to_string())
}

// ── status-bar signals (#111) ───────────────────────────────────────────────

/// The active connection's display name, falling back to its host.
fn conn_label(st: &State) -> String {
    st.active
        .as_ref()
        .and_then(|id| st.connections.iter().find(|c| &c.id == id))
        .map(|c| c.name.clone())
        .or_else(|| st.active_profile.as_ref().map(|p| p.host.clone()))
        .unwrap_or_default()
}

/// Emit the active connection's health (connecting / connected / error).
fn emit_conn(st: &State, status: SignalStatus) {
    signals::emit_signal("conn", &conn_label(st), status, 0);
}

/// Emit the active database name (queries + autocomplete target).
fn emit_db(st: &State) {
    if let Some(db) = st.active_profile.as_ref().map(|p| p.database.as_str()) {
        signals::emit_signal("db", db, SignalStatus::Ready, 0);
    }
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

    // Connections are shown grouped by environment; each group is a `conn-grp-<n>`
    // list whose row indices are group-local, so map them back to the global
    // `st.connections` index. A click opens, an action (pencil/trash) edits/deletes.
    if let Some(gi) = event
        .widget_id
        .strip_prefix("conn-grp-")
        .and_then(|s| s.parse::<usize>().ok())
    {
        let groups = crate::ui::connections::connection_groups(st);
        let global = |li: usize| groups.get(gi).and_then(|g| g.2.get(li)).copied();
        match event.kind.as_str() {
            "click" => {
                if let Some(conn) = event
                    .value
                    .parse::<usize>()
                    .ok()
                    .and_then(global)
                    .and_then(|i| st.connections.get(i).cloned())
                {
                    // Just activate it — the Schema tab then shows its tables.
                    activate_connection(st, &conn);
                }
            }
            "action" => {
                if let Ok(v) = serde_json::from_str::<Value>(&event.value) {
                    let item = v.get("item").and_then(|x| x.as_u64()).map(|i| i as usize);
                    let action = v.get("action").and_then(|x| x.as_u64()).unwrap_or(0);
                    if let Some(i) = item.and_then(global) {
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

    // The dialog is rendered on two surfaces (editor tab + sidebar panel); the
    // sidebar copy scopes its ids with an "sb-" prefix to avoid egui id
    // collisions. Strip it so both surfaces route to the same handlers.
    let widget_id = event
        .widget_id
        .strip_prefix("sb-")
        .unwrap_or(&event.widget_id);

    match widget_id {
        // dialog form fields (also accept bare ids so the integration test can
        // populate a profile without going through the dialog)
        "f-name" => st.form.name = parse_str(&event.value),
        "f-engine" => apply_engine_defaults(st, engine_from_value(&parse_str(&event.value))),
        "f-host" | "host" => st.form.host = parse_str(&event.value),
        "f-port" | "port" => st.form.port = parse_str(&event.value),
        "f-database" | "database" => st.form.database = parse_str(&event.value),
        "f-user" | "user" => st.form.user = parse_str(&event.value),
        "f-password" | "password" => st.form.password = parse_str(&event.value),
        "f-tls" | "tls" => st.form.tls = serde_json::from_str(&event.value).unwrap_or(false),
        "f-color" if event.kind == "change" => st.form.color = parse_str(&event.value),

        "new-connection" => {
            st.editing = None;
            st.form = Form::default();
            st.test_status = None;
            st.dialog_open = true;
            st.dialog_form_step = false; // start on the engine picker
        }
        // Engine picked on step 0 (a row in the engine list) → seed engine +
        // default port, advance to the credentials form.
        "engine-list" if event.kind == "click" => {
            if let Some(&engine) = event
                .value
                .parse::<usize>()
                .ok()
                .and_then(|i| crate::state::SUPPORTED_ENGINES.get(i))
            {
                apply_engine_defaults(st, engine);
                st.dialog_form_step = true;
            }
        }
        // Back to the engine picker.
        "dialog-back" => st.dialog_form_step = false,
        "dialog-close" | "dialog-cancel" => {
            st.dialog_open = false;
            st.editing = None;
            st.test_status = None;
        }
        "error-close" => st.error = None,
        "new-query" => {
            // Open a blank editor tab for the active connection.
            if let Some(conn) = st
                .active
                .as_deref()
                .and_then(|id| st.connections.iter().find(|c| c.id == id))
            {
                open_tab(
                    &conn.name,
                    &conn.id,
                    active_password(st, &conn.id),
                    None,
                    None,
                    false,
                );
            }
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
            st.databases.clear();
            st.databases_loaded = false;
            st.schema_error = None;
        }
        // Schema-tree data-rows, addressed by their path of indices:
        //   db:<i>            database          — toggles its schemas
        //   sch:<i>:<j>       schema            — toggles its tables
        //   tbl:<i>:<j>:<k>   table/view        — caret ("toggle") expands
        //                                         columns; a body "click" opens a SELECT
        //   col:<i>:<j>:<k>:<l>  column (leaf)  — clicking opens its table's SELECT
        "tree-more" => st.tree_limit = st.tree_limit.saturating_add(crate::state::TREE_PAGE),
        // Schema-browser server-side filter: text change runs a `FindTables`
        // search against the active database (deduped per keystroke); empty
        // restores the tree.
        "schema-filter" if event.kind == "change" => {
            set_schema_filter(st, &parse_str(&event.value))
        }
        // A server-side filter result row — open that table's data.
        id if id.starts_with("find:") => {
            if let Ok(i) = id[5..].parse::<usize>() {
                open_filter_match(st, i);
            }
        }
        id if id.starts_with("db:") => {
            if let Ok(i) = id[3..].parse::<usize>() {
                toggle_database(st, i);
            }
        }
        id if id.starts_with("sch:") => {
            if let [i, j] = parse_indices(&id[4..])[..] {
                toggle_schema(st, i, j);
            }
        }
        id if id.starts_with("tbl:") => {
            if let [i, j, k] = parse_indices(&id[4..])[..] {
                match event.kind.as_str() {
                    "toggle" => toggle_table(st, i, j, k),
                    "action" => open_structure_tab(st, i, j, k),
                    _ => open_table_data(st, i, j, k),
                }
            }
        }
        id if id.starts_with("col:") => {
            if let [i, j, k, ..] = parse_indices(&id[4..])[..] {
                open_table_data(st, i, j, k);
            }
        }
        // Editor-tab connection switcher: re-point this tab at another saved
        // connection. Keeps the SQL text; resets results and reloads the schema
        // (so autocomplete reflects the new target).
        // Only "change" (an option pick) acts; the searchable dropdown's "search"
        // events filter client-side (all names are already loaded), so ignore them.
        "switch-connection" if event.kind == "change" => {
            let id = parse_str(&event.value);
            if st.active.as_deref() != Some(id.as_str()) {
                if let Some(conn) = st.connections.iter().find(|c| c.id == id).cloned() {
                    activate_connection(st, &conn);
                }
            }
        }
        // Editor-tab database switcher: re-point this tab's queries + autocomplete
        // at another database in the current connection.
        "switch-database" if event.kind == "change" => {
            select_database(st, &parse_str(&event.value))
        }
        // Editor events: "change" carries the new SQL; "run" is a keyboard
        // shortcut (⌘Enter = statement at caret / selection, ⌘⇧Enter = all);
        // "run-marker" is a ▶ gutter click carrying a statement's char offset;
        // "format-editor" is the ⌥⇧F format shortcut (the SDK emits it on the
        // editor's id, unlike the toolbar button which emits its own click).
        "sql" => match event.kind.as_str() {
            "change" => st.sql = parse_str(&event.value),
            "run" => run_editor(st, &event.value),
            "format-editor" => format_query(st),
            "run-marker" => {
                if let Ok(offset) = event.value.parse::<usize>() {
                    if let Some(text) = sql::statement_at(&st.sql, offset) {
                        run_query_text(st, text);
                    }
                }
            }
            _ => {}
        },
        // Toolbar Run button: run the whole script.
        "run" => run_query(st),
        // Results footer "Load more": fetch the next page of the last-run query.
        "load-more" => load_more(st),
        // Results/Explain tab switch. Track the active tab so a later run knows
        // whether to refresh Explain; opening Explain lazily runs EXPLAIN ANALYZE
        // for the last-run query (it executes the query, so only on demand).
        "query-output" if event.kind == "change" => {
            if parse_str(&event.value) == "Explain" {
                st.results_tab = ResultsTab::Explain;
                load_explain(st);
            } else {
                st.results_tab = ResultsTab::Results;
            }
        }
        // Save the current SQL to a .sql file the user picks (native dialog).
        "save-query" => save_query(st),
        // Load a .sql file the user picks into the editor.
        "open-query" => open_query(st),
        "format-editor" => format_query(st),
        // Publish the current result to the host Datasets registry.
        "publish-dataset" => publish_result(st),
        _ => {}
    }
}

/// Publish the last successful query result as a dataset the host registry can
/// list/preview (and other plugins can read).
fn publish_result(st: &mut State) {
    let Some(Ok(value)) = st.result.as_ref() else {
        return;
    };
    let (Some(cols), Some(rows)) = (
        value.get("columns").and_then(|c| c.as_array()),
        value.get("rows").and_then(|r| r.as_array()),
    ) else {
        return;
    };

    let columns: Vec<DatasetColumn> = cols
        .iter()
        .map(|c| DatasetColumn {
            name: c
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string(),
            type_hint: c
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect();
    let data_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            row.as_array()
                .map(|cs| cs.iter().map(cell_to_string).collect())
                .unwrap_or_default()
        })
        .collect();

    // Name from the first line of the run SQL; tags carry connection + database.
    let name = st
        .last_run_sql
        .as_deref()
        .and_then(|s| s.trim().lines().next())
        .map(|line| line.chars().take(48).collect::<String>())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "query result".to_string());
    let mut tags = Vec::new();
    if let Some(conn) = st
        .active
        .as_ref()
        .and_then(|id| st.connections.iter().find(|c| &c.id == id))
    {
        tags.push(conn.name.clone());
    }
    if let Some(p) = st.active_profile.as_ref() {
        tags.push(p.database.clone());
    }

    let dataset = Dataset {
        name,
        kind: "sql-result".to_string(),
        tags,
        columns,
        rows: data_rows,
    };
    if let Err(e) = datasets::publish(&dataset) {
        st.error = Some(format!("Publish failed: {}", e.message));
    }
}

/// Render a result cell JSON value as a plain string for the dataset payload.
fn cell_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn format_query(st: &mut State) {
    use sqlformat::{format, FormatOptions, QueryParams};
    st.sql = format(&st.sql, &QueryParams::default(), &FormatOptions::default());
}

/// Save the editor's SQL to a `.sql` file via the host's native save dialog.
fn save_query(st: &mut State) {
    match file_dialog::save_file("Save query", "query.sql", &["sql".to_string()], &st.sql) {
        Ok(_) => {} // Ok(Some(path)) saved, Ok(None) cancelled — nothing to update.
        Err(e) => st.error = Some(format!("Couldn't save the query: {}", e.message)),
    }
}

/// Open a `.sql` file via the host's native open dialog into the editor.
fn open_query(st: &mut State) {
    match file_dialog::open_file("Open SQL file", &["sql".to_string()]) {
        Ok(Some(file)) => {
            st.sql = file.contents;
            // The plan no longer matches the loaded SQL.
            st.explain = None;
            st.explain_for = None;
        }
        Ok(None) => {} // cancelled
        Err(e) => st.error = Some(format!("Couldn't open the file: {}", e.message)),
    }
}

/// Run the whole editor script against the active connection. The Run button.
fn run_query(st: &mut State) {
    let sql = st.sql.clone();
    run_query_text(st, sql);
}

/// Handle an editor "run" shortcut: ⌘⇧Enter runs everything; otherwise run the
/// selection if there is one, else the statement under the caret.
fn run_editor(st: &mut State, value: &str) {
    let v: Value = serde_json::from_str(value).unwrap_or_default();
    if v.get("all").and_then(|b| b.as_bool()).unwrap_or(false) {
        run_query(st);
        return;
    }
    if let Some(sel) = v.get("selection").and_then(|s| s.as_array()) {
        if let [a, b] = &sel[..] {
            let (a, b) = (
                a.as_u64().unwrap_or(0) as usize,
                b.as_u64().unwrap_or(0) as usize,
            );
            let text = sql::slice(&st.sql, a, b);
            if !text.is_empty() {
                run_query_text(st, text);
                return;
            }
        }
    }
    let caret = v.get("caret").and_then(|c| c.as_u64()).unwrap_or(0) as usize;
    if let Some(text) = sql::statement_at(&st.sql, caret) {
        run_query_text(st, text);
    }
}

/// Run a specific SQL string against the active connection (no-op while a query
/// is already in flight). Records history and invalidates the cached plan.
fn run_query_text(st: &mut State, sql: String) {
    if st.loading || sql.trim().is_empty() {
        return;
    }
    st.last_run_sql = Some(sql.clone());
    st.row_limit = crate::state::ROW_PAGE; // fresh run — reset the cap
                                           // Invalidate any previous plan. If the Explain tab is showing, refresh it now
                                           // for this query (below); otherwise it's re-run lazily on tab open.
    st.explain = None;
    st.explain_for = None;
    st.explain_loading = false;
    if let Some(id) = st.active.clone() {
        record_history(&id, &sql);
    }
    execute_current(st);
    if st.results_tab == ResultsTab::Explain {
        load_explain(st);
    }
}

/// Fetch more rows of the last-run query by growing its cap and re-running it.
fn load_more(st: &mut State) {
    if st.loading || st.last_run_sql.is_none() {
        return;
    }
    st.row_limit = st.row_limit.saturating_add(crate::state::ROW_PAGE);
    execute_current(st);
}

/// Submit the last-run query with the current row cap. A cappable SELECT gets a
/// `LIMIT row_limit + 1` appended so the extra row signals "more available";
/// anything else runs unchanged. Shared by fresh runs and "Load more".
fn execute_current(st: &mut State) {
    let Some(base) = st.last_run_sql.clone() else {
        return;
    };
    st.loading = true;
    st.result = None;
    st.query_started = Some(std::time::Instant::now());
    // Push a "running" signal to the host status bar; the result handler
    // overwrites it with the row count + latency (Ready) or an Error.
    signals::emit_signal("rows", "", SignalStatus::Loading, 0);
    let (sql, limited) = match sql::add_limit(&base, st.row_limit + 1) {
        Some(capped) => (capped, true),
        None => (base, false),
    };
    st.run_limited = limited;
    submit(&Request::Query { sql }, Kind::Query, st);
}

/// Lazily run `EXPLAIN ANALYZE` for the **last-run** query — triggered when the
/// user opens the Explain tab, or runs a query while it's already open. Cached
/// per-SQL so it only re-executes when the last-run query changed. No-op until a
/// query has actually been run. Engine-specific via [`Engine::explain_sql`].
fn load_explain(st: &mut State) {
    let Some(sql) = st.last_run_sql.as_ref().map(|s| s.trim().to_string()) else {
        return;
    };
    if sql.is_empty() {
        return;
    }
    // Already have (or are fetching) the plan for this exact SQL.
    if st.explain_for.as_deref() == Some(sql.as_str())
        && (st.explain.is_some() || st.explain_loading)
    {
        return;
    }
    st.explain = None;
    st.explain_loading = true;
    st.explain_for = Some(sql.clone());
    let explain_sql = st.engine().explain_sql(&sql);
    submit(&Request::Query { sql: explain_sql }, Kind::QueryExplain, st);
}

/// Open an editor tab seeded with a connection (and optionally its password +
/// SQL). Passing the password lets the new instance skip a keychain read — and
/// therefore the macOS keychain prompt. When `run` is set, the tab executes the
/// seeded SQL on open (so it lands on the results grid).
fn open_tab(
    name: &str,
    conn_id: &str,
    password: Option<&str>,
    database: Option<&str>,
    sql: Option<&str>,
    run: bool,
) {
    let mut state = serde_json::json!({ "connection": conn_id });
    if let Some(p) = password {
        state["password"] = Value::from(p);
    }
    if let Some(db) = database {
        state["database"] = Value::from(db);
    }
    if let Some(s) = sql {
        state["sql"] = Value::from(s);
    }
    if run {
        state["run"] = Value::from(true);
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
    st.databases.clear();
    st.tree_limit = crate::state::TREE_PAGE;
    st.databases_loaded = false;
    st.schema_error = None;
    st.failed = None;
    st.error = None;
    load_databases(st);
}

// ── schema browser ────────────────────────────────────────────────────────────

/// Parse a `:`-separated path of tree indices (e.g. `"1:0:3"` → `[1, 0, 3]`).
fn parse_indices(s: &str) -> Vec<usize> {
    s.split(':').filter_map(|p| p.parse().ok()).collect()
}

/// Kick off the database list for the active connection (once). Listing
/// databases doubles as the connection probe on select.
fn load_databases(st: &mut State) {
    if st.databases_loaded {
        return;
    }
    st.databases_loaded = true;
    st.schema_error = None;
    // Listing databases doubles as the connection probe; show "connecting".
    emit_conn(st, SignalStatus::Loading);
    submit(&Request::ListDatabases, Kind::Databases, st);
}

/// Eagerly load tables for the next schema in `database` that doesn't have them
/// yet — one at a time. The `Kind::Tables` handler calls this again, so the
/// active database's tables load sequentially (for autocomplete) without ever
/// spawning a storm of concurrent query workers that would block the UI.
fn load_next_pending_tables(st: &mut State, database: &str) {
    let next = st
        .databases
        .iter()
        .find(|d| d.name == database)
        .and_then(|d| d.schemas.as_ref())
        .and_then(|ss| ss.iter().find(|s| s.tables.is_none()))
        .map(|s| s.name.clone());
    if let Some(schema) = next {
        submit(
            &Request::ListTables {
                database: database.to_string(),
                schema: schema.clone(),
            },
            Kind::Tables {
                database: database.to_string(),
                schema,
            },
            st,
        );
    }
}

/// Switch this instance's active database (queries + autocomplete target it).
/// Loads the database's schemas/tables for autocomplete if not already cached.
/// No-op if it's already active or there's no active connection.
fn select_database(st: &mut State, database: &str) {
    match st.active_profile.as_mut() {
        Some(p) if p.database != database => p.database = database.to_string(),
        _ => return,
    }
    // The previous database's results no longer apply.
    st.result = None;
    emit_db(st);

    // Make sure the new database's tables are loaded for autocomplete. If its
    // schemas aren't fetched yet, request them (the Schemas handler kicks off the
    // sequential table load for the now-active database); otherwise resume that
    // sequential load for any schema still missing tables.
    let schemas_loaded = st
        .databases
        .iter()
        .find(|d| d.name == database)
        .map(|d| d.schemas.is_some())
        .unwrap_or(false);
    if !schemas_loaded {
        submit(
            &Request::ListSchemas {
                database: database.to_string(),
            },
            Kind::Schemas {
                database: database.to_string(),
            },
            st,
        );
    } else {
        load_next_pending_tables(st, database);
    }
}

fn toggle_database(st: &mut State, i: usize) {
    let Some(db) = st.databases.get_mut(i) else {
        return;
    };
    db.expanded = !db.expanded;
    let need_load = db.expanded && db.schemas.is_none();
    let database = db.name.clone();
    if need_load {
        submit(
            &Request::ListSchemas {
                database: database.clone(),
            },
            Kind::Schemas { database },
            st,
        );
    }
}

fn toggle_schema(st: &mut State, i: usize, j: usize) {
    let Some(db) = st.databases.get_mut(i) else {
        return;
    };
    let database = db.name.clone();
    let Some(sch) = db.schemas.as_mut().and_then(|ss| ss.get_mut(j)) else {
        return;
    };
    sch.expanded = !sch.expanded;
    let need_load = sch.expanded && sch.tables.is_none();
    let schema = sch.name.clone();
    if need_load {
        submit(
            &Request::ListTables {
                database: database.clone(),
                schema: schema.clone(),
            },
            Kind::Tables { database, schema },
            st,
        );
    }
}

fn toggle_table(st: &mut State, i: usize, j: usize, k: usize) {
    let Some(db) = st.databases.get_mut(i) else {
        return;
    };
    let database = db.name.clone();
    let Some(sch) = db.schemas.as_mut().and_then(|ss| ss.get_mut(j)) else {
        return;
    };
    let schema = sch.name.clone();
    let Some(tbl) = sch.tables.as_mut().and_then(|t| t.get_mut(k)) else {
        return;
    };
    tbl.expanded = !tbl.expanded;
    let need_load = tbl.expanded && tbl.columns.is_none();
    let table = tbl.name.clone();
    if need_load {
        submit(
            &Request::ListColumns {
                database: database.clone(),
                schema: schema.clone(),
                table: table.clone(),
            },
            Kind::Columns {
                database,
                schema,
                table,
            },
            st,
        );
    }
}

/// Open a table's data in a new editor tab: `SELECT *` run immediately so the
/// tab lands on the results grid (a "view data" action). The query runs against
/// the connection's default database (browse-only), so this is meaningful for
/// tables in that database.
fn open_table_data(st: &State, i: usize, j: usize, k: usize) {
    let target = st
        .databases
        .get(i)
        .and_then(|db| db.schemas.as_ref())
        .and_then(|ss| ss.get(j))
        .and_then(|sch| {
            sch.tables
                .as_ref()
                .and_then(|t| t.get(k))
                .map(|tbl| (sch.name.clone(), tbl.name.clone()))
        });
    let Some((schema, table)) = target else {
        return;
    };
    open_table_data_named(st, &schema, &table);
}

/// Minimum query length before a server-side schema search runs (a 1-char
/// `LIKE %x%` on a huge catalog like Ensembl matches almost everything).
const SCHEMA_FILTER_MIN: usize = 2;

/// Update the schema-filter text; empty/too-short restores the tree, otherwise
/// (maybe) start a search. The search is server-side across the connection's
/// databases (MySQL server-wide, Postgres per-database).
fn set_schema_filter(st: &mut State, text: &str) {
    st.schema_filter = text.to_string();
    if st.schema_filter.trim().chars().count() < SCHEMA_FILTER_MIN {
        st.schema_matches = None;
        st.schema_searching = false;
        st.schema_filter_submitted.clear();
        return;
    }
    maybe_start_filter_search(st);
}

/// Start a `FindTables` search only if none is in flight — this coalesces rapid
/// keystrokes into a single request at a time (each catalog scan on a large DB
/// is slow, so firing one per keystroke would pile up and stall). When the
/// in-flight search returns, the result handler calls this again to run a
/// trailing search if the text moved on. No-op when the current text was already
/// searched.
fn maybe_start_filter_search(st: &mut State) {
    let query = st.schema_filter.trim().to_string();
    if query.chars().count() < SCHEMA_FILTER_MIN || st.schema_searching {
        return;
    }
    if query == st.schema_filter_submitted && st.schema_matches.is_some() {
        return;
    }
    st.schema_filter_submitted = query.clone();
    st.schema_searching = true;
    submit(&Request::FindTables { query }, Kind::FindTables, st);
}

/// Open a server-side filter match (by its display index) as a data tab. The
/// match carries its own database, so this opens against that database — MySQL
/// qualifies the table in a single connection, Postgres opens a tab connected to
/// the match's database (a PG connection can't query across databases).
fn open_filter_match(st: &State, index: usize) {
    let Some(m) = st
        .schema_matches
        .as_ref()
        .and_then(|v| v.get(index))
        .cloned()
    else {
        return;
    };
    let Some(conn) = st
        .active
        .as_deref()
        .and_then(|id| st.connections.iter().find(|c| c.id == id))
    else {
        return;
    };
    // No LIMIT here — the run path caps rows and offers "Load more" (matching
    // open_table_data_named).
    let (target_db, sql) = if conn.engine == crate::db::Engine::Mysql {
        // MySQL: the schema is the database; qualify in the current connection.
        let db = m.database.clone().unwrap_or_else(|| m.schema.clone());
        let db = db.replace('`', "``");
        let table = m.name.replace('`', "``");
        (None, format!("SELECT * FROM `{db}`.`{table}`"))
    } else {
        // Postgres: open a tab connected to the match's database.
        let schema = m.schema.replace('"', "\"\"");
        let table = m.name.replace('"', "\"\"");
        (
            m.database.clone(),
            format!("SELECT * FROM \"{schema}\".\"{table}\""),
        )
    };
    open_tab(
        &conn.name,
        &conn.id,
        active_password(st, &conn.id),
        target_db.as_deref(),
        Some(&sql),
        true,
    );
}

/// Open a table's structure (columns) in a new read-only tab.
fn open_structure_tab(st: &State, i: usize, j: usize, k: usize) {
    let target = st
        .databases
        .get(i)
        .and_then(|db| db.schemas.as_ref())
        .and_then(|ss| ss.get(j))
        .and_then(|sch| {
            sch.tables
                .as_ref()
                .and_then(|t| t.get(k))
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
    // The database being browsed (the connection's configured database).
    let database = st
        .active_profile
        .as_ref()
        .map(|p| p.database.clone())
        .unwrap_or_else(|| conn.database.clone());
    let mut state = serde_json::json!({
        "connection": conn.id,
        "view": "structure",
        "database": database,
        "schema": schema,
        "table": table,
    });
    if let Some(p) = active_password(st, &conn.id) {
        state["password"] = Value::from(p);
    }
    ui_tabs::open_tab(&table, Some(crate::ICON_TABLE), Some(&state.to_string()));
}

/// Open a table (by schema + name) as a `SELECT *` data tab against the active
/// connection's browsed database. Shared by the tree and the filter results.
fn open_table_data_named(st: &State, schema: &str, table: &str) {
    let Some(conn) = st
        .active
        .as_deref()
        .and_then(|id| st.connections.iter().find(|c| c.id == id))
    else {
        return;
    };
    // No LIMIT here — the run path caps rows and offers "Load more".
    let sql = if conn.engine == crate::db::Engine::Mysql {
        let schema = schema.replace('`', "``");
        let table = table.replace('`', "``");
        format!("SELECT * FROM `{schema}`.`{table}`")
    } else {
        let schema = schema.replace('"', "\"\"");
        let table = table.replace('"', "\"\"");
        format!("SELECT * FROM \"{schema}\".\"{table}\"")
    };
    open_tab(
        &conn.name,
        &conn.id,
        active_password(st, &conn.id),
        None,
        Some(&sql),
        true,
    );
}

/// Reopen a recent query in a fresh editor tab and run it.
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
        None,
        Some(&entry.sql),
        true,
    );
}

/// Seed an editor-tab instance from its initial-state blob
/// (`{connection, password?, sql?}`). Uses a handed-in password when present to
/// avoid a keychain read, falling back to the keychain otherwise. Loads the
/// schema/table list in the background (schemas live per-instance, and this tab
/// is a separate instance from the sidebar) so the editor's autocomplete knows
/// the connection's table names.
pub(crate) fn activate_from_state(st: &mut State, state: &str) {
    let Ok(v) = serde_json::from_str::<Value>(state) else {
        return;
    };
    // A structure tab: a read-only columns view for one table, not the editor.
    if v.get("view").and_then(|x| x.as_str()) == Some("structure") {
        activate_structure(st, &v);
        return;
    }
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
        // Restore the previously-selected database (if the snapshot carried one),
        // so reopening a tab keeps its database — not just the connection default.
        if let Some(database) = v.get("database").and_then(|d| d.as_str()) {
            if !database.is_empty() {
                if let Some(p) = st.active_profile.as_mut() {
                    p.database = database.to_string();
                }
            }
        }
        // Fetch databases (then the active database's schemas/tables) for this
        // instance so the editor's autocomplete is populated — the sidebar's
        // copy lives in a different instance.
        load_databases(st);
    }
    if let Some(sql) = v.get("sql").and_then(|s| s.as_str()) {
        if !sql.is_empty() {
            st.sql = sql.to_string();
        }
    }
    // Auto-run the seeded query (table "view data" / recent-query reopen) through
    // the normal run path so it gets the row cap + "Load more".
    if v.get("run").and_then(|r| r.as_bool()).unwrap_or(false) && !st.sql.is_empty() {
        run_query_text(st, st.sql.clone());
    }
}

/// Seed a structure-tab instance: set its connection/profile and kick off the
/// `ListColumns` describe for the target table. Doesn't load the schema tree or
/// autocomplete — a structure tab only needs the one table's columns.
fn activate_structure(st: &mut State, v: &Value) {
    let Some(conn) = v
        .get("connection")
        .and_then(|c| c.as_str())
        .and_then(|id| st.connections.iter().find(|c| c.id == id).cloned())
    else {
        return;
    };
    let (Some(schema), Some(table)) = (
        v.get("schema").and_then(|x| x.as_str()).map(String::from),
        v.get("table").and_then(|x| x.as_str()).map(String::from),
    ) else {
        return;
    };
    let database = v
        .get("database")
        .and_then(|x| x.as_str())
        .map(String::from)
        .unwrap_or_else(|| conn.database.clone());
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
        database: database.clone(),
        user: conn.user.clone(),
        password,
        tls: conn.tls,
    });
    st.active = Some(conn.id);
    st.view = crate::state::View::Structure {
        database: database.clone(),
        schema: schema.clone(),
        table: table.clone(),
    };
    st.structure = None;
    submit(
        &Request::DescribeTable {
            database,
            schema,
            table,
        },
        Kind::Structure,
        st,
    );
}

/// Seed the connection form with an engine's defaults (port, user, database)
/// from its [`DbAdapter`](crate::db::DbAdapter).
fn apply_engine_defaults(st: &mut State, engine: crate::db::Engine) {
    st.form.engine = engine;
    let d = crate::db::adapter(engine).connection_defaults();
    st.form.port = d.port.to_string();
    st.form.user = d.user.to_string();
    st.form.database = d.database.to_string();
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
        color: conn.color.clone().unwrap_or_default(),
    };
    st.editing = Some(conn.id);
    st.test_status = None;
    st.dialog_open = true;
    st.dialog_form_step = true; // editing skips the engine picker
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
    let color = Some(st.form.color.trim())
        .filter(|c| !c.is_empty())
        .map(str::to_string);
    let conn = Connection {
        id: id.clone(),
        name,
        engine: st.form.engine,
        host: profile.host.clone(),
        port: profile.port,
        database: profile.database.clone(),
        user: profile.user.clone(),
        tls: profile.tls,
        color,
    };
    // Persist the password to the keychain first; if that fails we must NOT save
    // the connection, or it would be unusable (no stored credential). Surface the
    // error and bail out, leaving the connections list untouched.
    if let Err(e) = secure_storage::write(&pw_key(&id), &st.form.password) {
        st.error = Some(format!(
            "Couldn't save the password to the keychain: {}",
            e.message
        ));
        return;
    }
    st.password_cache
        .insert(id.clone(), st.form.password.clone());
    match st.connections.iter_mut().find(|c| c.id == id) {
        Some(existing) => *existing = conn.clone(),
        None => st.connections.push(conn.clone()),
    }
    save_connections(st);

    st.editing = None;
    st.dialog_open = false;
    st.test_status = None;
    // Make the saved connection active (loads its schema), same as clicking it.
    activate_connection(st, &conn);
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
            st.has_more = false;
            st.result = Some(match (ok, err) {
                (Some(v), _) => {
                    let mut value = decode_inner(v);
                    // A capped run fetches `row_limit + 1` rows; the extra one is
                    // the "there are more" sentinel — drop it and flag Load more.
                    if st.run_limited {
                        if let Some(rows) = value.get_mut("rows").and_then(|r| r.as_array_mut()) {
                            if rows.len() > st.row_limit {
                                rows.truncate(st.row_limit);
                                st.has_more = true;
                            }
                        }
                    }
                    Ok(value)
                }
                (None, Some(m)) => Err(m),
                _ => Err("query failed".into()),
            });
            // Surface the outcome as a status-bar signal: the returned row count
            // (with a "+" when more rows are available) or an error state.
            let elapsed_ms = st.query_started.take().map(|t| t.elapsed().as_millis());
            match &st.result {
                Some(Ok(value)) => {
                    let n = value
                        .get("rows")
                        .and_then(|r| r.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    let count = if st.has_more {
                        format!("{n}+")
                    } else {
                        n.to_string()
                    };
                    // Include end-to-end latency when available: "100+ · 12 ms".
                    let shown = match elapsed_ms {
                        Some(ms) => format!("{count} · {ms} ms"),
                        None => count,
                    };
                    signals::emit_signal("rows", &shown, SignalStatus::Ready, 0);
                }
                Some(Err(_)) => signals::emit_signal("rows", "", SignalStatus::Error, 0),
                None => {}
            }
        }
        Kind::QueryExplain => {
            st.explain_loading = false;
            st.explain = Some(match (ok, err) {
                (Some(v), _) => Ok(decode_inner(v)),
                (None, Some(m)) => Err(m),
                _ => Err("query failed".into()),
            });
        }
        Kind::Databases => match (ok, err) {
            (Some(v), _) => {
                let names = decode_str_array(v);
                st.databases = names
                    .iter()
                    .map(|name| DatabaseNode {
                        name: name.clone(),
                        expanded: false,
                        schemas: None,
                    })
                    .collect();
                st.schema_error = None;
                st.error = None;
                st.failed = None;
                // Connection probe succeeded: surface connected + active database.
                emit_conn(st, SignalStatus::Ready);
                emit_db(st);
                // Load the connection's configured database's schemas (then, in
                // the Schemas handler, its tables — one at a time) so the SQL
                // editor's autocomplete is populated. The tree stays collapsed:
                // the user expands databases themselves. Other databases load
                // lazily on expand. Browse-only: queries run against this database.
                let default_db = st
                    .active_profile
                    .as_ref()
                    .map(|p| p.database.clone())
                    .unwrap_or_default();
                if st.databases.iter().any(|d| d.name == default_db) {
                    submit(
                        &Request::ListSchemas {
                            database: default_db.clone(),
                        },
                        Kind::Schemas {
                            database: default_db,
                        },
                        st,
                    );
                }
            }
            (None, m) => {
                // Listing databases is our connection probe on select. On failure,
                // surface it in the error modal and mark the connection as errored
                // instead of leaving it active.
                let msg = m.unwrap_or_else(|| "failed to connect".into());
                // Emit the error signal before clearing `active` (needed for the label).
                emit_conn(st, SignalStatus::Error);
                st.failed = st.active.take();
                st.active_profile = None;
                st.databases.clear();
                st.schema_error = Some(msg.clone());
                st.error = Some(msg);
            }
        },
        Kind::Schemas { database } => {
            match (ok, err) {
                (Some(v), _) => {
                    let names = decode_str_array(v);
                    if let Some(db) = st.databases.iter_mut().find(|d| d.name == database) {
                        db.schemas = Some(
                            names
                                .iter()
                                .map(|name| SchemaNode {
                                    name: name.clone(),
                                    expanded: false,
                                    tables: None,
                                })
                                .collect(),
                        );
                    }
                    st.schema_error = None;
                    // Eagerly fetch tables (one schema at a time — the Tables
                    // handler chains to the next) so autocomplete has table names
                    // and the tree fills in. For the active database always, and
                    // for MySQL always (its single synthetic schema has no
                    // separate toggle to load tables on).
                    let is_default = st
                        .active_profile
                        .as_ref()
                        .is_some_and(|p| p.database == database);
                    if is_default || st.engine() == crate::db::Engine::Mysql {
                        load_next_pending_tables(st, &database);
                    }
                }
                (None, m) => {
                    if let Some(m) = m {
                        st.schema_error = Some(m);
                    }
                }
            }
        }
        Kind::Tables { database, schema } => {
            let tables: Vec<TableInfo> = ok
                .map(|v| serde_json::from_value(decode_inner(v)).unwrap_or_default())
                .unwrap_or_default();
            if let Some(node) = st
                .databases
                .iter_mut()
                .find(|d| d.name == database)
                .and_then(|d| d.schemas.as_mut())
                .and_then(|ss| ss.iter_mut().find(|s| s.name == schema))
            {
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
            // Continue the sequential eager-load of the active database's tables
            // (for autocomplete). `node.tables` was set above (even on error), so
            // this advances to the next not-yet-loaded schema rather than looping.
            let is_default = st
                .active_profile
                .as_ref()
                .is_some_and(|p| p.database == database);
            if is_default {
                load_next_pending_tables(st, &database);
            }
        }
        Kind::Columns {
            database,
            schema,
            table,
        } => {
            let cols: Vec<ColumnInfo> = ok
                .map(|v| serde_json::from_value(decode_inner(v)).unwrap_or_default())
                .unwrap_or_default();
            if let Some(node) = st
                .databases
                .iter_mut()
                .find(|d| d.name == database)
                .and_then(|d| d.schemas.as_mut())
                .and_then(|ss| ss.iter_mut().find(|s| s.name == schema))
                .and_then(|s| s.tables.as_mut())
                .and_then(|ts| ts.iter_mut().find(|t| t.name == table))
            {
                node.columns = Some(cols);
            }
        }
        Kind::FindTables => {
            st.schema_searching = false;
            match (ok, err) {
                (Some(v), _) => {
                    let tables: Vec<TableInfo> =
                        serde_json::from_value(decode_inner(v)).unwrap_or_default();
                    st.schema_matches = Some(tables);
                    st.schema_error = None;
                }
                (None, m) => {
                    st.schema_matches = Some(Vec::new());
                    if let Some(m) = m {
                        st.schema_error = Some(m);
                    }
                }
            }
            // The user may have typed more while this was in flight — run the
            // trailing search now (coalesced).
            maybe_start_filter_search(st);
        }
        Kind::Structure => {
            st.structure = Some(match (ok, err) {
                (Some(v), _) => Ok(serde_json::from_value(decode_inner(v)).unwrap_or_default()),
                (None, Some(m)) => Err(m),
                _ => Err("failed to describe table".into()),
            });
        }
    }
}

/// Decode a host `ok` payload that wraps a JSON array of strings (database or
/// schema names) into a `Vec<String>`.
fn decode_str_array(v: &Value) -> Vec<String> {
    decode_inner(v)
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
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
