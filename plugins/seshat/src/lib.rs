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
use bindings::thoth::plugin::{db_runtime, types::Capability};
use db::Engine;

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

struct Seshat;

/// Connection form + query state. In WASM (single-threaded) this thread-local is
/// effectively a global shared by the UI-thread (handle_event) and the host's
/// query worker (query), which the host serializes via the Store mutex.
#[derive(Clone, Default)]
struct State {
    engine: Engine,
    host: String,
    port: String,
    database: String,
    user: String,
    password: String,
    tls: bool,
    sql: String,

    loading: bool,
    pending_request_id: Option<String>,
    /// Last result: Ok(rows-json) or Err(message).
    result: Option<Result<Value, String>>,
}

impl State {
    fn fresh() -> Self {
        Self {
            engine: Engine::Postgres,
            host: "localhost".into(),
            port: "5432".into(),
            database: "postgres".into(),
            user: "postgres".into(),
            sql: "SELECT 1 AS one;".into(),
            ..Default::default()
        }
    }

    fn profile(&self) -> db::Profile {
        db::Profile {
            host: self.host.clone(),
            port: self.port.trim().parse().unwrap_or(5432),
            database: self.database.clone(),
            user: self.user.clone(),
            password: self.password.clone(),
            tls: self.tls,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::fresh());
}

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

/// Serialize an adapter result to a JSON string, mapping both the DB error and
/// any serialization error into a `PluginError`.
fn to_json<T: Serialize>(result: Result<T, String>) -> Result<String, PluginError> {
    let value = result.map_err(|e| err(1, e))?;
    serde_json::to_string(&value).map_err(|e| err(3, e.to_string()))
}

// ── meta / lifecycle / settings ─────────────────────────────────────────────

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
            icon: Some("\u{E1D2}".to_string()), // database glyph
        }
    }
}

impl LifecycleGuest for Seshat {
    fn on_load(_setting: String) {}
    fn on_close() {
        STATE.with(|s| *s.borrow_mut() = State::fresh());
    }
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
        Some("\u{E1D2}".to_string())
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

// ── data-source: query() does the actual DB work (run on a host worker) ─────

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

    /// Dispatch one [`Request`] (SQL or introspection) against the current
    /// profile and return its JSON result. Called by the host on a worker
    /// thread via db-runtime::submit-query, so blocking DB I/O is off the UI.
    fn query(_handle: String, q: String) -> Result<String, PluginError> {
        let (profile, engine) = STATE.with(|s| {
            let st = s.borrow();
            (st.profile(), st.engine)
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

// ── ui-component: connect form + SQL + results ──────────────────────────────

impl UiComponentGuest for Seshat {
    fn render_sidebar() -> Result<Option<UiOutput>, PluginError> {
        Ok(Some(ui_out(json!({
            "type": "column",
            "gap": 0,
            "children": [
                {"type":"row","padding":6,"children":[
                    {"type":"heading","value":"SESHAT","panel":true}
                ]},
                {"type":"column","padding":8,"gap":6,"children":[
                    {"type":"text","value":"Connect from the editor tab.","muted":true}
                ]}
            ]
        }))))
    }

    fn render_ui() -> Result<UiOutput, PluginError> {
        STATE.with(|s| Ok(ui_out(build_ui(&s.borrow()))))
    }

    fn handle_event(event: UiEvent) -> Result<UiOutput, PluginError> {
        STATE.with(|s| {
            let mut st = s.borrow().clone();
            if event.kind == "query-result" {
                handle_query_result(&mut st, &event);
            } else {
                match event.widget_id.as_str() {
                    "host" => st.host = parse_str(&event.value),
                    "port" => st.port = parse_str(&event.value),
                    "database" => st.database = parse_str(&event.value),
                    "user" => st.user = parse_str(&event.value),
                    "password" => st.password = parse_str(&event.value),
                    "tls" => st.tls = serde_json::from_str(&event.value).unwrap_or(false),
                    "sql" => st.sql = parse_str(&event.value),
                    "run" => {
                        *s.borrow_mut() = st.clone(); // persist before the worker reads STATE
                        let req = serde_json::to_string(&Request::Query {
                            sql: st.sql.clone(),
                        })
                        .unwrap_or_default();
                        let id = db_runtime::submit_query("seshat", &req);
                        st.pending_request_id = Some(id);
                        st.loading = true;
                        st.result = None;
                    }
                    _ => {}
                }
            }
            *s.borrow_mut() = st.clone();
            Ok(ui_out(build_ui(&st)))
        })
    }
}

fn handle_query_result(st: &mut State, event: &UiEvent) {
    if st.pending_request_id.as_deref() != Some(&event.widget_id) {
        return;
    }
    st.loading = false;
    st.pending_request_id = None;
    let parsed: Value = serde_json::from_str(&event.value).unwrap_or_default();
    if let Some(ok) = parsed.get("ok") {
        // ok is the rows-JSON string the host wrapped from query()'s return.
        let rows: Value = ok
            .as_str()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| ok.clone());
        st.result = Some(Ok(rows));
    } else if let Some(e) = parsed.get("err") {
        let msg = e
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("query failed")
            .to_string();
        st.result = Some(Err(msg));
    }
}

fn build_ui(st: &State) -> Value {
    let results = if st.loading {
        json!({"type":"column","gap":8,"padding":12,"children":[
            {"type":"spinner"},
            {"type":"text","value":"Running query…","muted":true}
        ]})
    } else {
        match &st.result {
            Some(Ok(rows)) => json!({"type":"json-tree","value": rows}),
            Some(Err(msg)) => json!({"type":"colored","color":"#f38ba8",
                "child":{"type":"text","value": format!("Error: {msg}")}}),
            None => json!({"type":"text","value":"Run a query to see results.","muted":true}),
        }
    };

    json!({
        "type":"column","gap":0,"children":[
            {"type":"row","gap":6,"align":"fill","padding":6,"children":[
                {"type":"text-input","id":"host","value":st.host,"label":"","placeholder":"host","grow":true},
                {"type":"text-input","id":"port","value":st.port,"label":"","placeholder":"port"},
                {"type":"text-input","id":"database","value":st.database,"label":"","placeholder":"database"},
                {"type":"text-input","id":"user","value":st.user,"label":"","placeholder":"user"},
                {"type":"password-input","id":"password","value":st.password,"label":""},
                {"type":"checkbox","id":"tls","label":"TLS","checked":st.tls}
            ]},
            {"type":"separator"},
            {"type":"column","gap":6,"padding":6,"children":[
                {"type":"code-editor","id":"sql","value":st.sql},
                {"type":"row","gap":6,"children":[
                    {"type":"button","id":"run","props":{
                        "label":"Run","button-type":"Elevated","color":"Primary",
                        "enabled": !st.loading
                    }}
                ]}
            ]},
            {"type":"separator"},
            {"type":"scroll","id":"results","child": results}
        ]
    })
}

bindings::export!(Seshat with_types_in bindings);
