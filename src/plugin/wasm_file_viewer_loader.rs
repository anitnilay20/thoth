use std::path::Path;
use std::sync::Mutex;

use serde_json::Value;
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{DirPerms, FilePerms, ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView};

use crate::error::{Result, ThothError};

/// Mirrors the WIT `display-mode` enum. Defined separately so the rest of the
/// host codebase doesn't depend on wasmtime bindgen internals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Table,
    Custom,
}

wasmtime::component::bindgen!({
    path: "wit/thoth-plugin.wit",
    world: "file-viewer-plugin",
});

pub(crate) struct PluginState {
    wasi: WasiCtx,
    table: ResourceTable,
}

impl wasmtime_wasi::WasiView for PluginState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

struct WasmViewerInner {
    store: Store<PluginState>,
    bindings: FileViewerPlugin,
}

pub struct WasmFileViewerLoader {
    inner: Mutex<WasmViewerInner>,
    record_count: usize,
}

impl WasmFileViewerLoader {
    pub fn open(engine: &Engine, wasm_path: &Path, file_path: &Path) -> Result<Self> {
        let parent_dir = file_path.parent().unwrap_or(Path::new("."));
        let parent_str = parent_dir.to_string_lossy();
        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .preopened_dir(
                parent_dir,
                parent_str.as_ref(),
                DirPerms::READ,
                FilePerms::READ,
            )
            .map_err(|e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            })?
            .build();
        let mut store = Store::new(
            engine,
            PluginState {
                wasi,
                table: ResourceTable::new(),
            },
        );
        store
            .set_fuel(u64::MAX / 2)
            .map_err(|e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            })?;

        let component =
            Component::from_file(engine, wasm_path).map_err(|e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            })?;

        let mut linker = Linker::<PluginState>::new(engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| {
            ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            }
        })?;

        let bindings =
            FileViewerPlugin::instantiate(&mut store, &component, &linker).map_err(|e| {
                ThothError::PluginLoadError {
                    path: wasm_path.to_path_buf(),
                    reason: e.to_string(),
                }
            })?;

        let path_str = file_path.to_string_lossy().to_string();
        let record_count = bindings
            .thoth_plugin_file_loader()
            .call_open(&mut store, &path_str)
            .map_err(|e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            })?
            .map_err(|e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.message,
            })? as usize;

        Ok(Self {
            inner: Mutex::new(WasmViewerInner { store, bindings }),
            record_count,
        })
    }

    pub fn len(&self) -> usize {
        self.record_count
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&mut self, idx: usize) -> Result<Value> {
        let WasmViewerInner { store, bindings } = self.inner.get_mut().unwrap();
        store
            .set_fuel(u64::MAX / 2)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?;
        let json = bindings
            .thoth_plugin_file_loader()
            .call_get(store, idx as u64)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })?;
        serde_json::from_str(&json).map_err(Into::into)
    }

    pub fn raw_bytes(&self, idx: usize) -> Result<Vec<u8>> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmViewerInner { store, bindings } = &mut *guard;
        store
            .set_fuel(u64::MAX / 2)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?;
        bindings
            .thoth_plugin_file_loader()
            .call_raw_bytes(store, idx as u64)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })
    }

    pub fn preferred_display(&mut self) -> DisplayMode {
        let WasmViewerInner { store, bindings } = self.inner.get_mut().unwrap();
        let _ = store.set_fuel(u64::MAX / 2);
        match bindings
            .thoth_plugin_file_viewer()
            .call_preferred_display(store)
        {
            Ok(exports::thoth::plugin::file_viewer::DisplayMode::Table) => DisplayMode::Table,
            Ok(exports::thoth::plugin::file_viewer::DisplayMode::Custom) => DisplayMode::Custom,
            Err(_) => DisplayMode::Table,
        }
    }

    pub fn column_headers(&mut self) -> Option<Vec<String>> {
        let WasmViewerInner { store, bindings } = self.inner.get_mut().unwrap();
        let _ = store.set_fuel(u64::MAX / 2);
        bindings
            .thoth_plugin_file_viewer()
            .call_column_headers(store)
            .ok()
            .flatten()
    }

    pub fn render_record(&mut self, record_json: &str) -> Result<String> {
        let WasmViewerInner { store, bindings } = self.inner.get_mut().unwrap();
        store
            .set_fuel(u64::MAX / 2)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?;
        let output = bindings
            .thoth_plugin_file_viewer()
            .call_render_record(store, record_json)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })?;
        Ok(output.node_json)
    }
}
