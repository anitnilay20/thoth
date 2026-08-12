//! WIT-backed adapter for a plugin's optional display-free CLI facet.
//!
//! The SDK's [`PluginCli`](thoth_plugin_sdk::cli::PluginCli) trait is an
//! authoring API. This module is the actual process boundary: schemas and
//! invocations cross the WASM component interface as versioned JSON payloads.

use std::{path::Path, sync::Mutex};

use thoth_plugin_sdk::cli::{CliInvocation, CliOutput, CliSchema, PluginCli};
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView};

use crate::error::{Result, ThothError};
use crate::settings::PluginSettingData;

wasmtime::component::bindgen!({
    path: "wit/thoth-plugin.wit",
    world: "cli-plugin",
});

const CLI_FUEL_BUDGET: u64 = 5_000_000_000;

struct CliState {
    wasi: WasiCtx,
    table: ResourceTable,
}

impl wasmtime_wasi::WasiView for CliState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

struct Inner {
    store: Store<CliState>,
    bindings: CliPlugin,
}

/// Live adapter whose calls are backed exclusively by `plugin-cli` WIT exports.
pub struct WasmCliLoader {
    inner: Mutex<Inner>,
    schema: CliSchema,
}

impl WasmCliLoader {
    /// Instantiate a component that exports the opt-in `cli-plugin` world and
    /// validate its schema before registering any clap commands.
    pub fn open(engine: &Engine, wasm_path: &Path, settings: &[PluginSettingData]) -> Result<Self> {
        let load_error = |reason: String| ThothError::PluginLoadError {
            path: wasm_path.to_path_buf(),
            reason,
        };
        let mut store = Store::new(
            engine,
            CliState {
                wasi: WasiCtxBuilder::new().inherit_stderr().build(),
                table: ResourceTable::new(),
            },
        );
        store
            .set_fuel(CLI_FUEL_BUDGET)
            .map_err(|error| load_error(error.to_string()))?;
        let component = Component::from_file(engine, wasm_path)
            .map_err(|error| load_error(error.to_string()))?;
        let mut linker = Linker::<CliState>::new(engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
            .map_err(|error| load_error(error.to_string()))?;
        // A combined plugin world can contain imports irrelevant to schema
        // discovery. They remain traps; a concrete adapter such as Seshat's
        // supplies the imports its CLI execution actually needs.
        linker
            .define_unknown_imports_as_traps(&component)
            .map_err(|error| load_error(error.to_string()))?;
        let bindings = CliPlugin::instantiate(&mut store, &component, &linker)
            .map_err(|error| load_error(error.to_string()))?;
        let settings_json = serde_json::to_string(settings)
            .map_err(|error| load_error(format!("failed to encode plugin settings: {error}")))?;
        bindings
            .thoth_plugin_plugin_lifecycle()
            .call_on_load(&mut store, &settings_json)
            .map_err(|error| load_error(error.to_string()))?;
        let schema_json = bindings
            .thoth_plugin_plugin_cli()
            .call_schema(&mut store)
            .map_err(|error| load_error(error.to_string()))?
            .map_err(|error| load_error(error.message))?;
        let schema: CliSchema = serde_json::from_str(&schema_json).map_err(|error| {
            load_error(format!("plugin returned an invalid CLI schema: {error}"))
        })?;
        schema.validate().map_err(|error| {
            load_error(format!("plugin returned an invalid CLI schema: {error}"))
        })?;

        Ok(Self {
            inner: Mutex::new(Inner { store, bindings }),
            schema,
        })
    }
}

impl PluginCli for WasmCliLoader {
    fn cli_schema(&self) -> CliSchema {
        self.schema.clone()
    }

    fn run_cli(&self, invocation: &CliInvocation) -> std::result::Result<CliOutput, String> {
        let invocation_json = serde_json::to_string(invocation)
            .map_err(|error| format!("failed to encode CLI invocation: {error}"))?;
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        let Inner { store, bindings } = &mut *inner;
        store
            .set_fuel(CLI_FUEL_BUDGET)
            .map_err(|error| format!("failed to refuel plugin: {error}"))?;
        let output_json = bindings
            .thoth_plugin_plugin_cli()
            .call_run(store, &invocation_json)
            .map_err(|error| format!("plugin CLI trapped: {error}"))?
            .map_err(|error| error.message)?;
        serde_json::from_str(&output_json)
            .map_err(|error| format!("plugin returned invalid CLI output: {error}"))
    }
}
