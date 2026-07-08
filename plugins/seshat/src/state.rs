//! Connection model, runtime + persisted state, and the async request envelope.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thoth_plugin_sdk::state::PluginState;

use crate::bindings::thoth::plugin::{db_runtime, plugin_storage};
use crate::db::{self, ColumnInfo, Engine, TableDetail, TableInfo};

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
    /// Optional environment colour (a semantic token like `error`/`warning`/
    /// `success`) shown as a left accent + status-dot tint, so prod vs local
    /// reads at a glance. `None` = no accent.
    #[serde(default)]
    pub color: Option<String>,
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
    /// Environment colour token (empty = none).
    pub color: String,
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
            color: String::new(),
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
                .unwrap_or_else(|_| db::adapter(self.engine).connection_defaults().port),
            database: self.database.clone(),
            user: self.user.clone(),
            password: self.password.clone(),
            tls: self.tls,
        }
    }
}

/// What a pending async request will produce, so `query-result` can be routed.
/// Schema-introspection kinds carry the `database` they target, because a
/// connection can browse multiple databases (Postgres reconnects per database).
#[derive(Clone, PartialEq)]
pub(crate) enum Kind {
    Query,
    QueryExplain,
    Test,
    Databases,
    Schemas {
        database: String,
    },
    Tables {
        database: String,
        schema: String,
    },
    Columns {
        database: String,
        schema: String,
        table: String,
    },
    /// Server-side schema-filter results (a flat list of matching tables).
    FindTables,
    /// Full table detail for a dedicated structure tab.
    Structure,
}

/// What an editor-tab instance is showing: the SQL editor, or a read-only
/// structure view for one table (opened from the schema tree's row action).
#[derive(Clone, PartialEq)]
pub(crate) enum View {
    Editor,
    Structure {
        database: String,
        schema: String,
        table: String,
    },
}

/// Which results-area tab is active, mirrored from the tab-change event so the
/// plugin knows whether the Explain view is currently showing.
#[derive(Clone, Copy, PartialEq, Default)]
pub(crate) enum ResultsTab {
    #[default]
    Results,
    Explain,
}

/// A database node in the browser tree (schemas loaded lazily on expand). The
/// server's default database is auto-expanded on connect.
#[derive(Clone)]
pub(crate) struct DatabaseNode {
    pub name: String,
    pub expanded: bool,
    /// `None` until this database's schemas have been fetched.
    pub schemas: Option<Vec<SchemaNode>>,
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

/// One past query, shown in the History tab.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct HistoryEntry {
    pub connection: String,
    pub sql: String,
}

// ── runtime state ─────────────────────────────────────────────────────────────

pub(crate) struct State {
    pub loaded: bool,
    pub connections: Vec<Connection>,
    /// Active connection id; `Some` ⇒ show the editor, `None` ⇒ show connections.
    pub active: Option<String>,
    /// In-memory profile (incl. password) used by `query`. Falls back to `form`.
    pub active_profile: Option<db::Profile>,

    // new/edit-connection dialog
    pub dialog_open: bool,
    /// Wizard step: `false` = pick a database engine, `true` = enter credentials.
    /// New connections start on the engine step; editing jumps to the form.
    pub dialog_form_step: bool,
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
    /// The SQL of the most recently executed query (a single statement, a
    /// selection, or the whole script) — what the results grid and Explain
    /// reflect, so Explain analyses exactly what was run, not the whole editor.
    pub last_run_sql: Option<String>,
    /// Current server-side row cap for the last-run query. Reset to [`ROW_PAGE`]
    /// on a fresh run; grown by [`ROW_PAGE`] on "Load more".
    pub row_limit: usize,
    /// True when the last run had a `LIMIT` appended (a cappable SELECT), so the
    /// result handler can detect the "more rows available" sentinel.
    pub run_limited: bool,
    /// True when the last-run query hit the row cap (there are more rows to load).
    pub has_more: bool,
    /// Which results tab is showing, so a run while on Explain refreshes it.
    pub results_tab: ResultsTab,
    pub explain: Option<Result<Value, String>>,
    /// The SQL that [`explain`](State::explain) was computed for, so it only
    /// re-runs `EXPLAIN ANALYZE` when the last-run query changed.
    pub explain_for: Option<String>,
    /// True while an `EXPLAIN ANALYZE` request is in flight.
    pub explain_loading: bool,

    // schema browser
    pub databases_loaded: bool,
    pub schema_error: Option<String>,
    pub databases: Vec<DatabaseNode>,
    /// How many rows to render per tree level (databases, tables). Servers like
    /// Ensembl expose thousands of databases; rendering them all at once stalls
    /// the UI, so we page — a "Show more" row bumps this by [`TREE_PAGE`].
    pub tree_limit: usize,
    /// Current schema-filter text; empty shows the tree, non-empty shows the
    /// server-side search results ([`schema_matches`](State::schema_matches)).
    pub schema_filter: String,
    /// The last filter text a `FindTables` request was submitted for (dedupes
    /// per-keystroke queries).
    pub schema_filter_submitted: String,
    /// `Some` once server-side filter results have arrived for the current query.
    pub schema_matches: Option<Vec<TableInfo>>,
    /// True while a `FindTables` request is in flight.
    pub schema_searching: bool,

    // structure view (a table's columns, when this tab is a structure tab)
    /// This instance's view mode — editor or a table's structure.
    pub view: View,
    /// The structure tab's detail (`None` = loading; `Err` = failed).
    pub structure: Option<Result<TableDetail, String>>,

    // query history (persisted; newest last)
    pub history: Vec<HistoryEntry>,

    /// A message shown in the error modal; `None` hides it.
    pub error: Option<String>,
    /// Id of the connection whose last connect attempt failed (shown as an
    /// "error" pill, and never left active).
    pub failed: Option<String>,
    /// Session cache of decrypted passwords (keyed by connection id) so we read
    /// the OS keychain at most once per connection per session — fewer prompts.
    pub password_cache: HashMap<String, String>,
}

impl Default for State {
    fn default() -> Self {
        State::fresh()
    }
}

impl State {
    pub(crate) fn fresh() -> Self {
        Self {
            loaded: false,
            connections: Vec::new(),
            active: None,
            active_profile: None,
            dialog_open: false,
            dialog_form_step: false,
            editing: None,
            form: Form::default(),
            test_status: None,
            sql: "SELECT 1 AS one;".into(),
            loading: false,
            pending: Vec::new(),
            result: None,
            last_run_sql: None,
            row_limit: ROW_PAGE,
            run_limited: false,
            has_more: false,
            results_tab: ResultsTab::Results,
            explain: None,
            explain_for: None,
            explain_loading: false,
            databases_loaded: false,
            schema_error: None,
            databases: Vec::new(),
            tree_limit: TREE_PAGE,
            schema_filter: String::new(),
            schema_filter_submitted: String::new(),
            schema_matches: None,
            schema_searching: false,
            view: View::Editor,
            structure: None,
            history: Vec::new(),
            error: None,
            failed: None,
            password_cache: HashMap::new(),
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

pub(crate) static STATE: PluginState<State> = PluginState::new();

// ── engine helpers ──────────────────────────────────────────────────────────

/// Engines offered in the connection dialog's engine picker, in display order.
/// The picker renders these; the click handler maps the row index back to one.
pub(crate) const SUPPORTED_ENGINES: [Engine; 2] = [Engine::Postgres, Engine::Mysql];

/// Rows rendered per tree level before a "Show more" row appears (and how many
/// each "Show more" click reveals).
pub(crate) const TREE_PAGE: usize = 200;

/// Rows fetched per query "page" — the server-side cap on a run, grown by this
/// much each "Load more" (keeps huge tables from streaming everything at once).
pub(crate) const ROW_PAGE: usize = 100;

pub(crate) fn engine_from_value(v: &str) -> Engine {
    match v {
        "mysql" => Engine::Mysql,
        _ => Engine::Postgres,
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

// ── persistence ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
struct Persisted {
    #[serde(default)]
    connections: Vec<Connection>,
    #[serde(default)]
    history: Vec<HistoryEntry>,
}

pub(crate) fn pw_key(id: &str) -> String {
    format!("conn:{id}")
}

fn read_persisted() -> Persisted {
    let raw = plugin_storage::read();
    if raw.is_empty() {
        return Persisted::default();
    }
    serde_json::from_str(&raw).unwrap_or_default()
}

fn write_persisted(p: &Persisted) {
    if let Ok(data) = serde_json::to_string(p) {
        let _ = plugin_storage::write(&data);
    }
}

/// Load saved connections + history from plugin-storage, once per instance.
pub(crate) fn load_state(st: &mut State) {
    if st.loaded {
        return;
    }
    reload_persisted(st);
}

/// Re-read connections + history from storage (keeps the always-visible sidebar
/// fresh when an editor-tab instance writes history).
pub(crate) fn reload_persisted(st: &mut State) {
    st.loaded = true;
    let p = read_persisted();
    st.connections = p.connections;
    st.history = p.history;
}

/// Persist the connections list (passwords stay in the keychain), preserving
/// stored history.
pub(crate) fn save_connections(st: &State) {
    let mut p = read_persisted();
    p.connections = st.connections.clone();
    write_persisted(&p);
}

/// Append a query to history (preserving stored connections); deduped against the
/// previous entry and capped. Read-modify-write so concurrent editor tabs don't
/// clobber each other's history.
pub(crate) fn record_history(connection: &str, sql: &str) {
    let sql = sql.trim();
    if sql.is_empty() {
        return;
    }
    let mut p = read_persisted();
    if p.history.last().map(|h| h.sql.as_str()) == Some(sql) {
        return;
    }
    p.history.push(HistoryEntry {
        connection: connection.to_string(),
        sql: sql.to_string(),
    });
    let len = p.history.len();
    if len > 100 {
        p.history.drain(0..len - 100);
    }
    write_persisted(&p);
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
    Query {
        sql: String,
    },
    TestConnection,
    ListDatabases,
    ListSchemas {
        database: String,
    },
    ListTables {
        database: String,
        schema: String,
    },
    FindTables {
        query: String,
    },
    DescribeTable {
        database: String,
        schema: String,
        table: String,
    },
    ListColumns {
        database: String,
        schema: String,
        table: String,
    },
}

/// Enqueue `req` on the host query worker and record the pending request id.
pub(crate) fn submit(req: &Request, kind: Kind, st: &mut State) {
    let payload = serde_json::to_string(req).unwrap_or_default();
    let id = db_runtime::submit_query("seshat", &payload);
    st.pending.push((id, kind));
}
