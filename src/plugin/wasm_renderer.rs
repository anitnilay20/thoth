//! On-demand runner for **data-renderer** plugins (#135).
//!
//! A renderer is stateless: the host owns the dataset, serialises it to a
//! records-json blob, and calls the plugin's `data-renderer.render` to get back
//! a serialized `RenderNode` tree (node-json) which the host then draws inside
//! the DataView. Like the exporter runner, an instance is created fresh per
//! invocation and dropped afterwards.

use std::path::Path;

use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView};

use crate::error::{Result, ThothError};

wasmtime::component::bindgen!({
    path: "wit/thoth-plugin.wit",
    world: "data-renderer-plugin",
});

/// Fuel budget for one render — bounded like the exporter so a runaway renderer
/// traps instead of hanging the UI (this runs on the paint path).
const RENDER_FUEL: u64 = 50_000_000_000;

struct RendererState {
    wasi: WasiCtx,
    table: ResourceTable,
}

impl wasmtime_wasi::WasiView for RendererState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// Instantiate the renderer at `wasm_path` and run it over `records_json`,
/// returning the serialized `RenderNode` tree (node-json) it produced.
pub fn run_render(engine: &Engine, wasm_path: &Path, records_json: &str) -> Result<String> {
    let load_err = |e: String| ThothError::PluginLoadError {
        path: wasm_path.to_path_buf(),
        reason: e,
    };

    let wasi = WasiCtxBuilder::new().inherit_stdio().build();
    let mut store = Store::new(
        engine,
        RendererState {
            wasi,
            table: ResourceTable::new(),
        },
    );
    store
        .set_fuel(RENDER_FUEL)
        .map_err(|e| load_err(e.to_string()))?;

    let component = Component::from_file(engine, wasm_path).map_err(|e| load_err(e.to_string()))?;

    let mut linker = Linker::<RendererState>::new(engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| load_err(e.to_string()))?;

    let bindings = DataRendererPlugin::instantiate(&mut store, &component, &linker)
        .map_err(|e| load_err(e.to_string()))?;

    bindings
        .thoth_plugin_data_renderer()
        .call_render(&mut store, records_json)
        .map_err(|e| ThothError::Unknown {
            message: e.to_string(),
        })?
        .map_err(|e| ThothError::Unknown { message: e.message })
}
