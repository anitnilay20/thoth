//! Connection model, runtime + persisted state, and the async request envelope.

use std::cell::RefCell;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::bindings::thoth::plugin::{db_runtime, plugin_storage};
use crate::db::{self, ColumnInfo, Engine};

// ── connection + form models ──────────────────────────────────────────────────

/// A saved connection's metadata. The password is NOT stored here — it lives in
/// the OS keychain via `secure-storage`, keyed by [`pw_key`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Connection {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub engine: Engine,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    #[serde(default)]
    pub tls: bool,
}

impl Connection {
    pub(crate) fn summary(&self) -> String {
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
pub(crate) struct Form {
    pub name: String,
    pub engine: Engine,
    pub host: String,
    pub port: String,
    pub database: String,
    pub user: String,
    pub password: String,
    pub tls: bool,
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
    pub(crate) fn profile(&self) -> db::Profile {
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
#[derive(Clone, PartialEq)]
pub(crate) enum Kind {
    Query,
    Test,
    Schemas,
    Tables { schema: String },
    Columns { schema: String, table: String },
}

/// A schema node in the browser tree (tables loaded lazily on expand).
#[derive(Clone)]
pub(crate) struct SchemaNode {
    pub name: String,
    pub expanded: bool,
    /// `None` until the tables have been fetched.
    pub tables: Option<Vec<TableNode>>,
}

/// A table/view node (columns loaded lazily on expand).
#[derive(Clone)]
pub(crate) struct TableNode {
    pub name: String,
    pub kind: String,
    pub expanded: bool,
    pub columns: Option<Vec<ColumnInfo>>,
}

// ── runtime state ─────────────────────────────────────────────────────────────

#[derive(Default)]
pub(crate) struct State {
    pub loaded: bool,
    pub connections: Vec<Connection>,
    /// Active connection id; `Some` ⇒ show the editor, `None` ⇒ show connections.
    pub active: Option<String>,
    /// In-memory profile (incl. password) used by `query`. Falls back to `form`.
    pub active_profile: Option<db::Profile>,

    // new/edit-connection dialog
    pub dialog_open: bool,
    /// `Some(id)` while editing an existing connection; `None` while creating.
    pub editing: Option<String>,
    pub form: Form,
    pub test_status: Option<Result<String, String>>,

    // SQL editor / results
    pub sql: String,
    pub loading: bool,
    /// In-flight async requests: `(request-id, kind)`. A Vec because schema
    /// introspection can run concurrently with (and alongside) a query.
    pub pending: Vec<(String, Kind)>,
    pub result: Option<Result<Value, String>>,

    // schema browser (editor-tab instance)
    pub schema_loaded: bool,
    pub schema_error: Option<String>,
    pub schemas: Vec<SchemaNode>,
}

impl State {
    pub(crate) fn fresh() -> Self {
        Self {
            sql: "SELECT 1 AS one;".into(),
            form: Form::default(),
            ..Default::default()
        }
    }

    /// Profile used by `query`: the active connection, or the form as a fallback
    /// (the latter keeps the headless integration test independent of the keychain).
    pub(crate) fn query_profile(&self) -> db::Profile {
        self.active_profile
            .clone()
            .unwrap_or_else(|| self.form.profile())
    }

    pub(crate) fn engine(&self) -> Engine {
        self.active
            .as_deref()
            .and_then(|id| self.connections.iter().find(|c| c.id == id))
            .map(|c| c.engine)
            .unwrap_or(self.form.engine)
    }
}

thread_local! {
    pub(crate) static STATE: RefCell<State> = RefCell::new(State::fresh());
}

// ── engine helpers ──────────────────────────────────────────────────────────

pub(crate) fn engine_from_value(v: &str) -> Engine {
    match v {
        "mysql" => Engine::Mysql,
        _ => Engine::Postgres,
    }
}
pub(crate) fn engine_value(e: Engine) -> &'static str {
    match e {
        Engine::Postgres => "postgres",
        Engine::Mysql => "mysql",
    }
}
pub(crate) fn engine_label(e: Engine) -> &'static str {
    match e {
        Engine::Postgres => "PostgreSQL",
        Engine::Mysql => "MySQL",
    }
}
pub(crate) fn engine_badge(e: Engine) -> (&'static str, &'static str) {
    match e {
        Engine::Postgres => ("PG", "blue"),
        Engine::Mysql => ("MY", "orange"),
    }
}
pub(crate) fn default_port(e: Engine) -> u16 {
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

pub(crate) fn pw_key(id: &str) -> String {
    format!("conn:{id}")
}

/// Load saved connections from plugin-storage (best-effort).
pub(crate) fn load_state(st: &mut State) {
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
pub(crate) fn save_state(st: &State) {
    let data = serde_json::to_string(&Persisted {
        connections: st.connections.clone(),
    })
    .unwrap_or_default();
    let _ = plugin_storage::write(&data);
}

/// A unique, slugified connection id derived from `name`.
pub(crate) fn make_id(name: &str, existing: &[Connection]) -> String {
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

// ── async request envelope ────────────────────────────────────────────────────

/// An off-thread operation the host runs via `db-runtime::submit-query`. Encoded
/// as JSON in the query string so introspection and SQL share one async path.
#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum Request {
    Query { sql: String },
    TestConnection,
    ListDatabases,
    ListSchemas,
    ListTables { schema: String },
    ListColumns { schema: String, table: String },
}

/// Enqueue `req` on the host query worker and record the pending request id.
pub(crate) fn submit(req: &Request, kind: Kind, st: &mut State) {
    let payload = serde_json::to_string(req).unwrap_or_default();
    let id = db_runtime::submit_query("seshat", &payload);
    st.pending.push((id, kind));
}
