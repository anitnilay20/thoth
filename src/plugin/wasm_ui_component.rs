//! Host runtime for the `ui-component-plugin` world.
//!
//! Unlike [`WasmDataSourceLoader`](crate::plugin::wasm_data_source::WasmDataSourceLoader)
//! (which has data-source + http-client), this loader hosts a *pure* interactive UI
//! plugin. It provides the `ui-tabs` and `plugin-storage` host imports and drives the
//! `ui-component` + `tab-host` exports, so the plugin can render in a dock tab, open
//! more tabs, expose a title/icon, and snapshot/restore its per-tab state.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::app::persistent_state::PersistentState;
use crate::error::{Result, ThothError};
use crate::plugin::plugin_ui_host::{PluginUiHost, TabOpenRequest};
use crate::plugin::render_node::{UiEvent, UiOutput};
use crate::settings::PluginSettingData;

/// Fuel budget per WASM call (matches the data-source loader).
const PLUGIN_FUEL_BUDGET: u64 = 5_000_000_000;

fn refuel(store: &mut Store<UiComponentPluginState>) -> Result<()> {
    store
        .set_fuel(PLUGIN_FUEL_BUDGET)
        .map_err(|e| ThothError::Unknown {
            message: format!("failed to set plugin fuel: {e}"),
        })
}

wasmtime::component::bindgen!({
    path: "wit/thoth-plugin.wit",
    world: "ui-component-plugin",
});

static TAB_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_tab_request_id() -> String {
    format!(
        "tab-{}",
        TAB_REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

// ── per-store state ───────────────────────────────────────────────────────────

struct UiComponentPluginState {
    wasi: WasiCtx,
    table: ResourceTable,
    plugin_id: String,
    /// Tab-open requests raised by the plugin via the `ui-tabs` import.
    tab_tx: std::sync::mpsc::Sender<TabOpenRequest>,
}

impl WasiView for UiComponentPluginState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl thoth::plugin::plugin_storage::Host for UiComponentPluginState {
    fn read(&mut self) -> String {
        match PersistentState::plugin_state_path(&self.plugin_id) {
            Ok(p) => std::fs::read_to_string(&p).unwrap_or_default(),
            Err(_) => String::new(),
        }
    }

    fn write(&mut self, data: String) -> std::result::Result<(), String> {
        let path =
            PersistentState::plugin_state_path(&self.plugin_id).map_err(|err| err.to_string())?;
        std::fs::write(&path, data.as_bytes()).map_err(|e| e.to_string())
    }
}

impl thoth::plugin::ui_tabs::Host for UiComponentPluginState {
    fn open_tab(
        &mut self,
        title: String,
        icon: Option<String>,
        initial_state: Option<String>,
    ) -> String {
        let request_id = next_tab_request_id();
        let _ = self.tab_tx.send(TabOpenRequest::sanitized(
            request_id.clone(),
            self.plugin_id.clone(),
            title,
            icon,
            initial_state,
        ));
        request_id
    }
}

// ── inner / outer structs ─────────────────────────────────────────────────────

struct WasmUiComponentInner {
    store: Store<UiComponentPluginState>,
    bindings: UiComponentPlugin,
}

pub struct WasmUiComponentLoader {
    inner: Mutex<WasmUiComponentInner>,
    tab_rx: std::sync::mpsc::Receiver<TabOpenRequest>,
    plugin_id: String,
}

impl WasmUiComponentLoader {
    pub fn open(
        engine: &Engine,
        wasm_path: &Path,
        plugin_id: String,
        settings: &[PluginSettingData],
    ) -> Result<Self> {
        let wasi = WasiCtxBuilder::new().inherit_stdio().build();
        let (tab_tx, tab_rx) = std::sync::mpsc::channel::<TabOpenRequest>();

        let state = UiComponentPluginState {
            wasi,
            table: ResourceTable::new(),
            plugin_id: plugin_id.clone(),
            tab_tx,
        };

        let mut store = Store::new(engine, state);
        refuel(&mut store).map_err(|e| ThothError::PluginLoadError {
            path: wasm_path.to_path_buf(),
            reason: e.to_string(),
        })?;

        let component =
            Component::from_file(engine, wasm_path).map_err(|e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            })?;

        let mut linker = Linker::<UiComponentPluginState>::new(engine);

        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| {
            ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            }
        })?;

        thoth::plugin::plugin_storage::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s).map_err(
            |e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            },
        )?;

        thoth::plugin::ui_tabs::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s).map_err(
            |e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            },
        )?;

        let bindings =
            UiComponentPlugin::instantiate(&mut store, &component, &linker).map_err(|e| {
                ThothError::PluginLoadError {
                    path: wasm_path.to_path_buf(),
                    reason: e.to_string(),
                }
            })?;

        let mut loader = Self {
            inner: Mutex::new(WasmUiComponentInner { store, bindings }),
            tab_rx,
            plugin_id,
        };

        loader.on_load(settings)?;
        Ok(loader)
    }

    pub fn on_load(&mut self, settings: &[PluginSettingData]) -> Result<()> {
        let settings_json = serde_json::to_string(settings).map_err(|e| ThothError::Unknown {
            message: format!("Failed to serialize plugin settings: {e}"),
        })?;
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmUiComponentInner { store, bindings } = &mut *guard;
        refuel(store)?;
        bindings
            .thoth_plugin_plugin_lifecycle()
            .call_on_load(store, &settings_json)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })
    }

    pub fn on_setting_change(&self, settings: &[PluginSettingData]) -> Result<()> {
        let settings_json = serde_json::to_string(settings).map_err(|e| ThothError::Unknown {
            message: format!("Failed to serialize plugin settings: {e}"),
        })?;
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmUiComponentInner { store, bindings } = &mut *guard;
        refuel(store)?;
        bindings
            .thoth_plugin_plugin_lifecycle()
            .call_on_setting_change(store, &settings_json)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })
    }

    pub fn render_ui(&self) -> Result<UiOutput> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmUiComponentInner { store, bindings } = &mut *guard;
        refuel(store)?;
        let wit_out = bindings
            .thoth_plugin_ui_component()
            .call_render_ui(store)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })?;
        Ok(UiOutput {
            node_json: wit_out.node_json,
            height_hint: wit_out.height_hint,
        })
    }

    pub fn handle_event(&self, event: UiEvent) -> Result<UiOutput> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmUiComponentInner { store, bindings } = &mut *guard;
        refuel(store)?;
        let wit_event = exports::thoth::plugin::ui_component::UiEvent {
            widget_id: event.widget_id,
            kind: event.kind,
            value: event.value,
        };
        let wit_out = bindings
            .thoth_plugin_ui_component()
            .call_handle_event(store, &wit_event)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })?;
        Ok(UiOutput {
            node_json: wit_out.node_json,
            height_hint: wit_out.height_hint,
        })
    }

    pub fn render_sidebar(&self) -> Result<Option<UiOutput>> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmUiComponentInner { store, bindings } = &mut *guard;
        refuel(store)?;
        let result = bindings
            .thoth_plugin_ui_component()
            .call_render_sidebar(store)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })?;
        Ok(result.map(|o| UiOutput {
            node_json: o.node_json,
            height_hint: o.height_hint,
        }))
    }

    // ── tab-host export ─────────────────────────────────────────────────────────

    pub fn tab_title(&self) -> Option<String> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmUiComponentInner { store, bindings } = &mut *guard;
        refuel(store).ok()?;
        bindings.thoth_plugin_tab_host().call_tab_title(store).ok()
    }

    pub fn tab_icon(&self) -> Option<String> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmUiComponentInner { store, bindings } = &mut *guard;
        refuel(store).ok()?;
        bindings
            .thoth_plugin_tab_host()
            .call_tab_icon(store)
            .ok()
            .flatten()
    }

    pub fn get_state(&self) -> Result<Option<String>> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmUiComponentInner { store, bindings } = &mut *guard;
        refuel(store)?;
        let blob = bindings
            .thoth_plugin_tab_host()
            .call_get_state(store)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })?;
        Ok(Some(blob))
    }

    pub fn init_with_state(&self, state: &str) -> Result<()> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmUiComponentInner { store, bindings } = &mut *guard;
        refuel(store)?;
        bindings
            .thoth_plugin_tab_host()
            .call_init_with_state(store, state)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })
    }

    fn call_lifecycle(&self, which: TabLifecycle) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmUiComponentInner { store, bindings } = &mut *guard;
        if refuel(store).is_err() {
            return;
        }
        let host = bindings.thoth_plugin_tab_host();
        let _ = match which {
            TabLifecycle::Focused => host.call_on_tab_focused(store),
            TabLifecycle::Blurred => host.call_on_tab_blurred(store),
            TabLifecycle::Closed => host.call_on_tab_closed(store),
        };
    }

    /// Non-blocking drain of tab-open requests raised during the last WASM call.
    pub fn drain_tab_open_requests(&self) -> Vec<TabOpenRequest> {
        let mut out = Vec::new();
        while let Ok(req) = self.tab_rx.try_recv() {
            out.push(req);
        }
        out
    }

    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }
}

enum TabLifecycle {
    Focused,
    Blurred,
    Closed,
}

impl PluginUiHost for WasmUiComponentLoader {
    fn plugin_id(&self) -> &str {
        WasmUiComponentLoader::plugin_id(self)
    }

    fn render_ui(&self) -> Result<UiOutput> {
        WasmUiComponentLoader::render_ui(self)
    }

    fn handle_event(&self, event: UiEvent) -> Result<UiOutput> {
        WasmUiComponentLoader::handle_event(self, event)
    }

    fn render_sidebar(&self) -> Result<Option<UiOutput>> {
        WasmUiComponentLoader::render_sidebar(self)
    }

    fn on_setting_change(&self, settings: &[PluginSettingData]) -> Result<()> {
        WasmUiComponentLoader::on_setting_change(self, settings)
    }

    fn tab_title(&self) -> Option<String> {
        WasmUiComponentLoader::tab_title(self).filter(|s| !s.is_empty())
    }

    fn tab_icon(&self) -> Option<String> {
        WasmUiComponentLoader::tab_icon(self)
    }

    fn get_state(&self) -> Result<Option<String>> {
        WasmUiComponentLoader::get_state(self)
    }

    fn init_with_state(&self, state: &str) -> Result<()> {
        WasmUiComponentLoader::init_with_state(self, state)
    }

    fn on_tab_focused(&self) {
        self.call_lifecycle(TabLifecycle::Focused);
    }

    fn on_tab_blurred(&self) {
        self.call_lifecycle(TabLifecycle::Blurred);
    }

    fn on_tab_closed(&self) {
        self.call_lifecycle(TabLifecycle::Closed);
    }

    fn drain_tab_open_requests(&self) -> Vec<TabOpenRequest> {
        WasmUiComponentLoader::drain_tab_open_requests(self)
    }
}
