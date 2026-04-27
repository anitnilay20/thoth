use std::path::Path;
use std::sync::Mutex;

use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView};

use crate::error::{Result, ThothError};
use crate::plugin::render_node::UiOutput;

wasmtime::component::bindgen!({
    path: "wit/thoth-plugin.wit",
    world: "base-plugin",
});

struct PluginSettingsState {
    wasi: WasiCtx,
    table: ResourceTable,
}

impl wasmtime_wasi::WasiView for PluginSettingsState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

struct Inner {
    store: Store<PluginSettingsState>,
    bindings: BasePlugin,
}

/// Thin wasmtime wrapper that instantiates any Thoth plugin solely to drive
/// its `plugin-settings` interface. Does not open files or make connections.
pub struct WasmPluginSettings {
    inner: Mutex<Inner>,
}

impl WasmPluginSettings {
    /// Instantiate the plugin at `wasm_path` for settings-only use.
    pub fn new(engine: &Engine, wasm_path: &Path) -> Result<Self> {
        let wasi = WasiCtxBuilder::new().inherit_stdio().build();
        let state = PluginSettingsState {
            wasi,
            table: ResourceTable::new(),
        };

        let mut store = Store::new(engine, state);
        store.set_fuel(u64::MAX / 2).ok();

        let component =
            Component::from_file(engine, wasm_path).map_err(|e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            })?;

        let mut linker: Linker<PluginSettingsState> = Linker::new(engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| {
            ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            }
        })?;

        linker
            .define_unknown_imports_as_traps(&component)
            .map_err(|e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            })?;

        let bindings = BasePlugin::instantiate(&mut store, &component, &linker).map_err(|e| {
            ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            }
        })?;

        Ok(Self {
            inner: Mutex::new(Inner { store, bindings }),
        })
    }

    /// Render the initial settings UI given the currently persisted values.
    pub fn render_settings(&self) -> Result<UiOutput> {
        let mut g = self.inner.lock().unwrap();
        let Inner { store, bindings } = &mut *g;
        store.set_fuel(u64::MAX / 2).ok();
        let result = bindings
            .thoth_plugin_plugin_settings()
            .call_render_settings(store)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })?;

        Ok(UiOutput {
            node_json: result.node_json,
            height_hint: result.height_hint,
        })
    }
}
