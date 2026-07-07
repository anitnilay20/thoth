#[rustfmt::skip]
mod bindings;
mod db;
mod events;
mod mysql;
mod pg;
mod shim;
mod sql;
mod state;
mod ui;
mod constants;

use serde::Serialize;
use serde_json::json;

use bindings::exports::thoth::plugin::{
    data_source::{ConfigEntry, Guest as DataSourceGuest, PaneOutput, PluginError, SourceSchema},
    plugin_lifecycle::Guest as LifecycleGuest,
    plugin_settings::{Guest as SettingsGuest, SettingsOutput},
    tab_host::Guest as TabHostGuest,
    ui_component::{Guest as UiComponentGuest, UiEvent, UiOutput},
};
use thoth_plugin_sdk::PluginMeta;

use events::apply_event;
use state::{load_state, reload_persisted, Request, STATE};
use ui::{build_sidebar, build_ui};

/// Phosphor (regular) glyphs, shared across the view modules.
pub(crate) const ICON_DATABASE: &str = "\u{E1DE}";
pub(crate) const ICON_PLUS: &str = "\u{E3D4}";
pub(crate) const ICON_PENCIL: &str = "\u{E3B4}";
pub(crate) const ICON_TRASH: &str = "\u{E4A6}";
pub(crate) const ICON_PLUG: &str = "\u{E946}";
pub(crate) const ICON_PLAY: &str = "\u{E3D0}";
pub(crate) const ICON_PLUGS_CONNECTED: &str = "\u{EB5A}";
pub(crate) const ICON_TREE_STRUCTURE: &str = "\u{E67C}";
pub(crate) const ICON_HISTORY: &str = "\u{E1A0}";
pub(crate) const ICON_TERMINAL: &str = "\u{EAE8}"; // TERMINAL_WINDOW — query editor tab
                                                   // schema-tree glyphs
pub(crate) const ICON_FOLDER: &str = "\u{E24A}";
pub(crate) const ICON_TABLE: &str = "\u{E476}";
pub(crate) const ICON_EYE: &str = "\u{E220}";
pub(crate) const ICON_KEY: &str = "\u{E2D6}";
pub(crate) const ICON_CIRCLE: &str = "\u{E18A}";
pub(crate) const ICON_FLOPPY_DISK: &str = "\u{E248}"; // save query to a .sql file
pub(crate) const ICON_FOLDER_OPEN: &str = "\u{E256}"; // open a .sql file
                                                      // structure-view glyphs
pub(crate) const ICON_LINK: &str = "\u{E2E2}"; // foreign key
pub(crate) const ICON_LIST_NUMBERS: &str = "\u{E2F6}"; // index
pub(crate) const ICON_FINGERPRINT: &str = "\u{E23E}"; // unique constraint
pub(crate) const ICON_CHECK_SQUARE: &str = "\u{E186}"; // check constraint
pub(crate) const ICON_LIGHTNING: &str = "\u{E2DE}"; // triggers (empty state)

#[derive(PluginMeta)]
#[plugin(
    id = "com.thoth.seshat",
    name = "Seshat",
    version = "0.1.0",
    description = "Database client for Thoth",
    capabilities = [DataSource, NewUiComponent],
    author = "Thoth contributors",
    icon = ICON_DATABASE,
)]
struct Seshat;

// ── shared helpers ────────────────────────────────────────────────────────────

fn ui_out(node: thoth_plugin_sdk::render_node::RenderNode) -> UiOutput {
    UiOutput {
        node_json: serde_json::to_string(&node).unwrap_or_default(),
        height_hint: 0,
    }
}

/// A plain text [`RenderNode`] (used for settings / empty placeholders).
fn text_node(value: &str) -> thoth_plugin_sdk::render_node::RenderNode {
    thoth_plugin_sdk::render_node::RenderNode::Text(
        thoth_plugin_sdk::components::Typography::builder()
            .text(value)
            .build(),
    )
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

// ── meta / lifecycle / settings / tab-host ───────────────────────────────────

impl LifecycleGuest for Seshat {
    fn on_load(_setting: String) {
        STATE.with_mut(load_state);
    }
    fn on_close() {
        // Drop in-memory runtime state (active_profile, password_cache) on
        // lifecycle close, matching url-source / csv-loader.
        STATE.reset();
    }
    fn on_setting_change(_setting: String) {}
}

impl SettingsGuest for Seshat {
    fn render_settings() -> Result<SettingsOutput, PluginError> {
        let node = text_node("No configurable settings yet.");
        Ok(SettingsOutput {
            node_json: serde_json::to_string(&node).unwrap_or_default(),
            height_hint: 0,
        })
    }
}

impl TabHostGuest for Seshat {
    fn tab_title() -> String {
        STATE.with(|st| {
            // A structure tab is titled after its table; an editor tab after its
            // connection.
            if let state::View::Structure { table, .. } = &st.view {
                return table.clone();
            }
            st.active
                .as_deref()
                .and_then(|id| st.connections.iter().find(|c| c.id == id))
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Seshat".to_string())
        })
    }
    fn tab_icon() -> Option<String> {
        // Structure tabs get the table glyph; editor tabs the terminal glyph.
        Some(
            STATE
                .with(|st| match st.view {
                    state::View::Structure { .. } => ICON_TABLE,
                    state::View::Editor => ICON_TERMINAL,
                })
                .to_string(),
        )
    }
    /// Snapshot the editor tab so the host can restore it across restarts.
    fn get_state() -> Result<String, PluginError> {
        Ok(STATE.with(|st| {
            match &st.view {
                // A structure tab restores back into the same table view.
                state::View::Structure {
                    database,
                    schema,
                    table,
                } => json!({
                    "connection": st.active,
                    "view": "structure",
                    "database": database,
                    "schema": schema,
                    "table": table,
                }),
                state::View::Editor => json!({
                    "connection": st.active,
                    "database": st.active_profile.as_ref().map(|p| p.database.clone()),
                    "sql": st.sql,
                }),
            }
            .to_string()
        }))
    }
    /// Seed a freshly-opened editor tab with its connection (and SQL).
    fn init_with_state(state: String) -> Result<(), PluginError> {
        STATE.with_mut(|st| {
            load_state(st);
            events::activate_from_state(st, &state);
        });
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
        let (profile, engine) = STATE.with(|st| (st.query_profile(), st.engine()));
        let adapter = db::adapter(engine);
        let req: Request =
            serde_json::from_str(&q).map_err(|e| err(2, format!("bad request: {e}")))?;
        // Queries and database listing use the connection's configured database;
        // schema/table/column introspection targets a specific database, so we
        // reconnect there by overriding `database` (Postgres can't introspect a
        // database other than the one it's connected to).
        match req {
            Request::Query { sql } => to_json(adapter.run_query(&profile, &sql)),
            Request::TestConnection => to_json(adapter.test_connection(&profile)),
            Request::ListDatabases => to_json(adapter.list_databases(&profile)),
            Request::ListSchemas { database } => {
                to_json(adapter.list_schemas(&db::Profile { database, ..profile }))
            }
            Request::ListTables { database, schema } => {
                to_json(adapter.list_tables(&db::Profile { database, ..profile }, &schema))
            }
            // Search scope is the adapter's concern (MySQL is server-wide;
            // Postgres iterates its databases), so query against the base profile.
            Request::FindTables { query } => to_json(adapter.find_tables(&profile, &query)),
            Request::DescribeTable {
                database,
                schema,
                table,
            } => to_json(adapter.describe_table(
                &db::Profile { database, ..profile },
                &schema,
                &table,
            )),
            Request::ListColumns {
                database,
                schema,
                table,
            } => to_json(adapter.list_columns(&db::Profile { database, ..profile }, &schema, &table)),
        }
    }

    fn close(_handle: String) {}

    fn render_pane(_handle: String) -> Result<PaneOutput, PluginError> {
        Ok(PaneOutput {
            node_json: serde_json::to_string(&text_node("")).unwrap_or_default(),
            height_hint: 0,
        })
    }
}

// ── ui-component ──────────────────────────────────────────────────────────────

impl UiComponentGuest for Seshat {
    fn render_sidebar() -> Result<Option<UiOutput>, PluginError> {
        STATE.with_mut(|st| {
            // Re-read persisted connections + history so entries written by editor
            // tabs (a separate instance) show up in the always-visible sidebar.
            reload_persisted(st);
            Ok(Some(ui_out(build_sidebar(st))))
        })
    }

    fn render_ui() -> Result<UiOutput, PluginError> {
        STATE.with_mut(|st| {
            load_state(st);
            Ok(ui_out(build_ui(st)))
        })
    }

    fn handle_event(event: UiEvent) -> Result<UiOutput, PluginError> {
        STATE.with_mut(|st| {
            load_state(st);
            apply_event(st, &event);
            Ok(ui_out(build_ui(st)))
        })
    }
}

bindings::export!(Seshat with_types_in bindings);
