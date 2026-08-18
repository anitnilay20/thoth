#[rustfmt::skip]
mod bindings;
mod cli;
mod constants;
mod db;
mod events;
mod icons;
mod query;
mod service;
mod shim;
mod sql;
mod state;
mod ui;

use serde::Serialize;
use serde_json::json;

use bindings::exports::thoth::plugin::{
    data_producer::{
        Dataset as ProducerDataset, DatasetColumn as ProducerColumn, Guest as DataProducerGuest,
        PluginError as ProducerError,
    },
    data_source::{ConfigEntry, Guest as DataSourceGuest, PaneOutput, PluginError, SourceSchema},
    plugin_cli::Guest as CliGuest,
    plugin_lifecycle::Guest as LifecycleGuest,
    plugin_settings::{Guest as SettingsGuest, SettingsOutput},
    tab_host::Guest as TabHostGuest,
    ui_component::{Guest as UiComponentGuest, UiEvent, UiOutput},
};
use thoth_plugin_sdk::PluginMeta;

use events::apply_event;
use state::{load_state, reload_persisted, Request, STATE};
use ui::{build_sidebar, build_ui};

#[derive(PluginMeta)]
#[plugin(
    id = "com.thoth.seshat",
    name = "Seshat",
    version = "0.1.0",
    description = "Database client for Thoth",
    capabilities = [DataSource, NewUiComponent, DataProducer, Cli],
    author = "Thoth contributors",
    icon = crate::constants::icons::ICON_DATABASE,
)]
struct Seshat;

impl CliGuest for Seshat {
    fn schema() -> Result<String, bindings::exports::thoth::plugin::plugin_cli::PluginError> {
        serde_json::to_string(&cli::schema()).map_err(|error| {
            bindings::exports::thoth::plugin::plugin_cli::PluginError {
                code: 1,
                message: error.to_string(),
            }
        })
    }

    fn run(
        invocation_json: String,
    ) -> Result<String, bindings::exports::thoth::plugin::plugin_cli::PluginError> {
        let invocation = serde_json::from_str(&invocation_json).map_err(|error| {
            bindings::exports::thoth::plugin::plugin_cli::PluginError {
                code: 2,
                message: format!("invalid CLI invocation: {error}"),
            }
        })?;
        let output = cli::run(invocation).map_err(|message| {
            bindings::exports::thoth::plugin::plugin_cli::PluginError { code: 3, message }
        })?;
        serde_json::to_string(&output).map_err(|error| {
            bindings::exports::thoth::plugin::plugin_cli::PluginError {
                code: 4,
                message: error.to_string(),
            }
        })
    }
}

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
                    state::View::Structure { .. } => crate::constants::icons::ICON_TABLE,
                    state::View::Editor => crate::constants::icons::ICON_TERMINAL,
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
        let req: Request =
            serde_json::from_str(&q).map_err(|e| err(2, format!("bad request: {e}")))?;
        // Queries and database listing use the connection's configured database;
        // schema/table/column introspection targets a specific database, so we
        // reconnect there by overriding `database` (Postgres can't introspect a
        // database other than the one it's connected to).
        match req {
            Request::Query { sql } => to_json(service::run_query(engine, &profile, &sql)),
            Request::TestConnection => to_json(service::test_connection(engine, &profile)),
            Request::ListDatabases => to_json(service::list_databases(engine, &profile)),
            Request::ListSchemas { database } => {
                to_json(db::adapter(engine).list_schemas(&db::Profile {
                    database,
                    ..profile
                }))
            }
            Request::ListTables { database, schema } => to_json(db::adapter(engine).list_tables(
                &db::Profile {
                    database,
                    ..profile
                },
                &schema,
            )),
            // Search scope is the adapter's concern (MySQL is server-wide;
            // Postgres iterates its databases), so query against the base profile.
            Request::FindTables { query } => {
                to_json(db::adapter(engine).find_tables(&profile, &query))
            }
            Request::DescribeTable {
                database,
                schema,
                table,
            } => to_json(db::adapter(engine).describe_table(
                &db::Profile {
                    database,
                    ..profile
                },
                &schema,
                &table,
            )),
            Request::ListColumns {
                database,
                schema,
                table,
            } => to_json(db::adapter(engine).list_columns(
                &db::Profile {
                    database,
                    ..profile
                },
                &schema,
                &table,
            )),
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

// ── data-producer: hand the current query result to the dataset bus ──────────

impl DataProducerGuest for Seshat {
    fn provide_dataset() -> Result<ProducerDataset, ProducerError> {
        STATE.with(|st| {
            let Some(Ok(value)) = st.result.as_ref() else {
                return Err(ProducerError {
                    code: 1,
                    message: "no query result to provide".to_string(),
                });
            };
            let (Some(cols), Some(rows)) = (
                value.get("columns").and_then(|c| c.as_array()),
                value.get("rows").and_then(|r| r.as_array()),
            ) else {
                return Err(ProducerError {
                    code: 1,
                    message: "result has no columns/rows".to_string(),
                });
            };
            let columns: Vec<ProducerColumn> = cols
                .iter()
                .map(|c| ProducerColumn {
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
            // Normalize every row to the column count so downstream column
            // indexing stays aligned: pad short/non-array rows with empty
            // cells and drop any extras.
            let width = columns.len();
            let rows = rows
                .iter()
                .map(|row| {
                    let mut cells: Vec<String> = row
                        .as_array()
                        .map(|cs| cs.iter().map(cell_to_string).collect())
                        .unwrap_or_default();
                    cells.resize(width, String::new());
                    cells
                })
                .collect();
            let name = st
                .last_run_sql
                .as_deref()
                .and_then(|s| s.trim().lines().next())
                .map(|line| line.chars().take(48).collect::<String>())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "query result".to_string());
            Ok(ProducerDataset {
                name,
                kind: "sql-result".to_string(),
                columns,
                rows,
            })
        })
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

bindings::export!(Seshat with_types_in bindings);
