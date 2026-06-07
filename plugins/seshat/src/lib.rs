#[rustfmt::skip]
mod bindings;
mod db;
mod pg;
mod shim;

use std::cell::RefCell;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use bindings::exports::thoth::plugin::{
    data_source::{ConfigEntry, Guest as DataSourceGuest, PaneOutput, PluginError, SourceSchema},
    plugin_lifecycle::Guest as LifecycleGuest,
    plugin_meta::Guest as MetaGuest,
    plugin_settings::{Guest as SettingsGuest, SettingsOutput},
    tab_host::Guest as TabHostGuest,
    ui_component::{Guest as UiComponentGuest, UiEvent, UiOutput},
};
use bindings::thoth::plugin::{db_runtime, plugin_storage, secure_storage, types::Capability};
use db::Engine;

// Phosphor (regular) glyphs.
const ICON_DATABASE: &str = "\u{E1DE}";
const ICON_PLUS: &str = "\u{E3D4}";
const ICON_CARET_LEFT: &str = "\u{E138}";
const ICON_TRASH: &str = "\u{E4A6}";
const ICON_PLUG: &str = "\u{E946}";
const ICON_PLAY: &str = "\u{E3D0}";

struct Seshat;

// ── persisted + runtime state ───────────────────────────────────────────────

/// A saved connection's metadata. The password is NOT stored here — it lives in
/// the OS keychain via `secure-storage`, keyed by [`pw_key`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Connection {
    id: String,
    name: String,
    #[serde(default)]
    engine: Engine,
    host: String,
    port: u16,
    database: String,
    user: String,
    #[serde(default)]
    tls: bool,
}

impl Connection {
    fn summary(&self) -> String {
        format!(
            "{} · {}@{}:{}/{}",
            engine_label(self.engine),
            if self.user.is_empty() {
                "—"
            } else {
                &self.user
            },
            self.host,
            self.port,
            self.database
        )
    }
}

/// The new-connection dialog form (port kept as a string while editing).
#[derive(Clone, Debug)]
struct Form {
    name: String,
    engine: Engine,
    host: String,
    port: String,
    database: String,
    user: String,
    password: String,
    tls: bool,
}

impl Default for Form {
    fn default() -> Self {
        Self {
            name: String::new(),
            engine: Engine::Postgres,
            host: "localhost".into(),
            port: "5432".into(),
            database: "postgres".into(),
            user: "postgres".into(),
            password: String::new(),
            tls: false,
        }
    }
}

impl Form {
    fn profile(&self) -> db::Profile {
        db::Profile {
            host: self.host.clone(),
            port: self
                .port
                .trim()
                .parse()
                .unwrap_or_else(|_| default_port(self.engine)),
            database: self.database.clone(),
            user: self.user.clone(),
            password: self.password.clone(),
            tls: self.tls,
        }
    }
}

/// What a pending async request will produce, so `query-result` can be routed.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Query,
    Test,
}

#[derive(Default)]
struct State {
    loaded: bool,
    connections: Vec<Connection>,
    /// Active connection id; `Some` ⇒ show the editor, `None` ⇒ show connections.
    active: Option<String>,
    /// In-memory profile (incl. password) used by `query`. Falls back to `form`.
    active_profile: Option<db::Profile>,

    // new-connection dialog
    dialog_open: bool,
    form: Form,
    test_status: Option<Result<String, String>>,

    // SQL editor / results
    sql: String,
    loading: bool,
    pending: Option<(String, Kind)>,
    result: Option<Result<Value, String>>,
}

impl State {
    fn fresh() -> Self {
        Self {
            sql: "SELECT 1 AS one;".into(),
            form: Form::default(),
            ..Default::default()
        }
    }

    /// Profile used by `query`: the active connection, or the form as a fallback
    /// (the latter keeps the headless integration test independent of the keychain).
    fn query_profile(&self) -> db::Profile {
        self.active_profile
            .clone()
            .unwrap_or_else(|| self.form.profile())
    }

    fn engine(&self) -> Engine {
        self.active
            .as_deref()
            .and_then(|id| self.connections.iter().find(|c| c.id == id))
            .map(|c| c.engine)
            .unwrap_or(self.form.engine)
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::fresh());
}

// ── engine helpers ──────────────────────────────────────────────────────────

fn engine_from_value(v: &str) -> Engine {
    match v {
        "mysql" => Engine::Mysql,
        _ => Engine::Postgres,
    }
}
fn engine_value(e: Engine) -> &'static str {
    match e {
        Engine::Postgres => "postgres",
        Engine::Mysql => "mysql",
    }
}
fn engine_label(e: Engine) -> &'static str {
    match e {
        Engine::Postgres => "PostgreSQL",
        Engine::Mysql => "MySQL",
    }
}
fn engine_badge(e: Engine) -> (&'static str, &'static str) {
    match e {
        Engine::Postgres => ("PG", "blue"),
        Engine::Mysql => ("MY", "orange"),
    }
}
fn default_port(e: Engine) -> u16 {
    match e {
        Engine::Postgres => 5432,
        Engine::Mysql => 3306,
    }
}

// ── persistence ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
struct Persisted {
    #[serde(default)]
    connections: Vec<Connection>,
}

fn pw_key(id: &str) -> String {
    format!("conn:{id}")
}

/// Load saved connections from plugin-storage (best-effort).
fn load_state(st: &mut State) {
    if st.loaded {
        return;
    }
    st.loaded = true;
    let raw = plugin_storage::read();
    if !raw.is_empty() {
        if let Ok(p) = serde_json::from_str::<Persisted>(&raw) {
            st.connections = p.connections;
        }
    }
}

/// Persist the connections list (metadata only — never the password).
fn save_state(st: &State) {
    let data = serde_json::to_string(&Persisted {
        connections: st.connections.clone(),
    })
    .unwrap_or_default();
    let _ = plugin_storage::write(&data);
}

/// A unique, slugified connection id derived from `name`.
fn make_id(name: &str, existing: &[Connection]) -> String {
    let base: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let base = base.trim_matches('-');
    let base = if base.is_empty() { "connection" } else { base };
    if !existing.iter().any(|c| c.id == base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !existing.iter().any(|c| &c.id == id))
        .unwrap()
}

// ── small JSON/UI helpers ─────────────────────────────────────────────────────

fn parse_str(s: &str) -> String {
    serde_json::from_str::<String>(s).unwrap_or_else(|_| s.to_string())
}

fn ui_out(node: Value) -> UiOutput {
    UiOutput {
        node_json: node.to_string(),
        height_hint: 0,
    }
}

fn err(code: u32, message: impl Into<String>) -> PluginError {
    PluginError {
        code,
        message: message.into(),
    }
}

/// Serialize an adapter result to a JSON string, mapping errors to `PluginError`.
fn to_json<T: Serialize>(result: Result<T, String>) -> Result<String, PluginError> {
    let value = result.map_err(|e| err(1, e))?;
    serde_json::to_string(&value).map_err(|e| err(3, e.to_string()))
}

/// An off-thread operation the host runs via `db-runtime::submit-query`. Encoded
/// as JSON in the query string so introspection and SQL share one async path.
#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Query { sql: String },
    TestConnection,
    ListDatabases,
    ListSchemas,
    ListTables { schema: String },
    ListColumns { schema: String, table: String },
}

fn submit(req: &Request, kind: Kind, st: &mut State) {
    let payload = serde_json::to_string(req).unwrap_or_default();
    let id = db_runtime::submit_query("seshat", &payload);
    st.pending = Some((id, kind));
}

// ── meta / lifecycle / settings / tab-host ───────────────────────────────────

impl MetaGuest for Seshat {
    fn get_info() -> bindings::exports::thoth::plugin::plugin_meta::PluginInfo {
        bindings::exports::thoth::plugin::plugin_meta::PluginInfo {
            id: "com.thoth.seshat".to_string(),
            name: "Seshat".to_string(),
            version: "0.1.0".to_string(),
            description: "Database client for Thoth".to_string(),
            capabilities: vec![Capability::DataSource, Capability::NewUiComponent],
            author: Some("Thoth contributors".to_string()),
            homepage: None,
            icon: Some(ICON_DATABASE.to_string()),
        }
    }
}

impl LifecycleGuest for Seshat {
    fn on_load(_setting: String) {
        STATE.with(|s| load_state(&mut s.borrow_mut()));
    }
    fn on_close() {}
    fn on_setting_change(_setting: String) {}
}

impl SettingsGuest for Seshat {
    fn render_settings() -> Result<SettingsOutput, PluginError> {
        Ok(SettingsOutput {
            node_json: json!({"type":"text","value":"No configurable settings yet.","muted":true})
                .to_string(),
            height_hint: 0,
        })
    }
}

impl TabHostGuest for Seshat {
    fn tab_title() -> String {
        "Seshat".to_string()
    }
    fn tab_icon() -> Option<String> {
        Some(ICON_DATABASE.to_string())
    }
    fn get_state() -> Result<String, PluginError> {
        Ok(String::new())
    }
    fn init_with_state(_state: String) -> Result<(), PluginError> {
        Ok(())
    }
    fn on_tab_focused() {}
    fn on_tab_blurred() {}
    fn on_tab_closed() {}
}

// ── data-source: query() runs on a host worker thread ─────────────────────────

impl DataSourceGuest for Seshat {
    fn required_config() -> Vec<ConfigEntry> {
        Vec::new()
    }
    fn connect(_config: Vec<ConfigEntry>) -> Result<String, PluginError> {
        Ok("seshat".to_string())
    }
    fn schema(_handle: String) -> Result<Vec<SourceSchema>, PluginError> {
        Ok(Vec::new())
    }

    /// Dispatch one [`Request`] against the active profile and return its JSON.
    fn query(_handle: String, q: String) -> Result<String, PluginError> {
        let (profile, engine) = STATE.with(|s| {
            let st = s.borrow();
            (st.query_profile(), st.engine())
        });
        let adapter = db::adapter(engine);
        let req: Request =
            serde_json::from_str(&q).map_err(|e| err(2, format!("bad request: {e}")))?;
        match req {
            Request::Query { sql } => to_json(adapter.run_query(&profile, &sql)),
            Request::TestConnection => to_json(adapter.test_connection(&profile)),
            Request::ListDatabases => to_json(adapter.list_databases(&profile)),
            Request::ListSchemas => to_json(adapter.list_schemas(&profile)),
            Request::ListTables { schema } => to_json(adapter.list_tables(&profile, &schema)),
            Request::ListColumns { schema, table } => {
                to_json(adapter.list_columns(&profile, &schema, &table))
            }
        }
    }

    fn close(_handle: String) {}

    fn render_pane(_handle: String) -> Result<PaneOutput, PluginError> {
        Ok(PaneOutput {
            node_json: json!({"type":"text","value":""}).to_string(),
            height_hint: 0,
        })
    }
}

// ── ui-component ──────────────────────────────────────────────────────────────

impl UiComponentGuest for Seshat {
    fn render_sidebar() -> Result<Option<UiOutput>, PluginError> {
        STATE.with(|s| {
            let mut st = s.borrow_mut();
            load_state(&mut st);
            Ok(Some(ui_out(build_sidebar(&st))))
        })
    }

    fn render_ui() -> Result<UiOutput, PluginError> {
        STATE.with(|s| {
            let mut st = s.borrow_mut();
            load_state(&mut st);
            Ok(ui_out(build_ui(&st)))
        })
    }

    fn handle_event(event: UiEvent) -> Result<UiOutput, PluginError> {
        STATE.with(|s| {
            let mut st = s.borrow_mut();
            load_state(&mut st);
            apply_event(&mut st, &event);
            Ok(ui_out(build_ui(&st)))
        })
    }
}

fn apply_event(st: &mut State, event: &UiEvent) {
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
                    open_connection(st, i);
                }
            }
            "action" => {
                if let Ok(v) = serde_json::from_str::<Value>(&event.value) {
                    if let Some(i) = v.get("item").and_then(|x| x.as_u64()) {
                        delete_connection(st, i as usize);
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
            st.form = Form::default();
            st.test_status = None;
            st.dialog_open = true;
        }
        "dialog-close" | "dialog-cancel" => {
            st.dialog_open = false;
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

fn open_connection(st: &mut State, index: usize) {
    let Some(conn) = st.connections.get(index).cloned() else {
        return;
    };
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
    st.active = Some(conn.id);
    st.result = None;
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

/// Save the dialog form as a new connection, store its password, and activate it.
fn connect_from_form(st: &mut State) {
    let name = if st.form.name.trim().is_empty() {
        st.form.host.clone()
    } else {
        st.form.name.trim().to_string()
    };
    let id = make_id(&name, &st.connections);
    let profile = st.form.profile();
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
    st.connections.push(conn);
    save_state(st);

    st.active_profile = Some(profile);
    st.active = Some(id);
    st.dialog_open = false;
    st.test_status = None;
    st.result = None;
}

fn handle_query_result(st: &mut State, event: &UiEvent) {
    let Some((req_id, kind)) = st.pending.clone() else {
        return;
    };
    if req_id != event.widget_id {
        return;
    }
    st.pending = None;
    let parsed: Value = serde_json::from_str(&event.value).unwrap_or_default();
    let ok = parsed.get("ok");
    let err = parsed
        .get("err")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .map(String::from);

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

bindings::export!(Seshat with_types_in bindings);

// ── view builders ─────────────────────────────────────────────────────────────
mod ui;
use ui::{build_sidebar, build_ui};
