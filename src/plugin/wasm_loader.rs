use std::path::Path;
use std::sync::Mutex;

use serde_json::Value;
use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{DirPerms, FilePerms, ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView};

use crate::error::{Result, ThothError};

wasmtime::component::bindgen!({
    path: "wit/thoth-plugin.wit",
    world: "file-loader-plugin",
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

struct WasmLoaderInner {
    store: Store<PluginState>,
    bindings: FileLoaderPlugin,
}

pub struct WasmFileLoader {
    inner: Mutex<WasmLoaderInner>,
    record_count: usize,
}

impl WasmFileLoader {
    pub fn open(engine: &Engine, wasm_path: &Path, file_path: &Path) -> Result<Self> {
        // Grant the plugin read access to the file's parent directory so it can
        // open the file via the WASI filesystem API.
        let parent_dir = file_path.parent().unwrap_or(Path::new("."));
        let parent_str = parent_dir.to_string_lossy();
        // Intentional WASI grants:
        //   inherit_stdio  — plugins may write debug output to stderr during development.
        //   preopened_dir  — plugins must be able to open the file being loaded; they are
        //                    sandboxed to the file's parent directory (read-only).
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
        // File-loader plugins read the entire file into WASM memory during open().
        // Use a very generous fuel budget — these run locally and the file size
        // is unbounded, so treating fuel as a hard cap here would break large files.
        let fuel_budget = u64::MAX / 2;
        store
            .set_fuel(fuel_budget)
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
            FileLoaderPlugin::instantiate(&mut store, &component, &linker).map_err(|e| {
                ThothError::PluginLoadError {
                    path: wasm_path.to_path_buf(),
                    reason: e.to_string(),
                }
            })?;

        // Call open() to index the file and get the total record count
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
            inner: Mutex::new(WasmLoaderInner { store, bindings }),
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
        let WasmLoaderInner { store, bindings } = self.inner.get_mut().unwrap();
        // Replenish fuel before each call to prevent exhaustion on large files
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

    /// Bulk sequential read of `[start, start + count)` — one WASM call.
    pub fn get_range(&mut self, start: usize, count: usize) -> Result<Vec<Value>> {
        let WasmLoaderInner { store, bindings } = self.inner.get_mut().unwrap();
        store
            .set_fuel(u64::MAX / 2)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?;
        let rows = bindings
            .thoth_plugin_file_loader()
            .call_get_range(store, start as u64, count as u64)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })?;
        rows.iter()
            .map(|json| serde_json::from_str(json).map_err(Into::into))
            .collect()
    }

    pub fn raw_bytes(&self, idx: usize) -> Result<Vec<u8>> {
        let mut guard = self.inner.lock().unwrap();
        let WasmLoaderInner { store, bindings } = &mut *guard;
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

    pub fn on_load(&mut self, settings_data: &str) -> Result<()> {
        let mut guard = self.inner.lock().unwrap();
        let WasmLoaderInner { store, bindings } = &mut *guard;

        store
            .set_fuel(u64::MAX / 2)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?;

        // Invoke the plugin lifecycle hook so plugins can initialize state.
        bindings
            .thoth_plugin_plugin_lifecycle()
            .call_on_load(store, settings_data)
            .map_err(|e| ThothError::PluginLoadError {
                path: Path::new("<plugin on_load>").to_path_buf(),
                reason: e.to_string(),
            })?;

        Ok(())
    }
}

impl Drop for WasmFileLoader {
    fn drop(&mut self) {
        let WasmLoaderInner { store, bindings } = self.inner.get_mut().unwrap();
        let _ = store.set_fuel(u64::MAX / 2);
        let _ = bindings
            .thoth_plugin_plugin_lifecycle()
            .call_on_close(store);
    }
}
