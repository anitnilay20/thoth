//! On-demand runner for **data-renderer** plugins (#135).
//!
//! A renderer is stateless: the host owns the dataset, hands the plugin a
//! handle, and calls `data-renderer.render`; the plugin pages the rows through
//! the `dataset-read` import and returns a serialized `RenderNode` tree
//! (node-json) which the host draws inside the DataView. Like the exporter
//! runner, an instance is created fresh per invocation and dropped afterwards.

use std::path::Path;

use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView};

use crate::error::{Result, ThothError};

wasmtime::component::bindgen!({
    path: "../wit/thoth-plugin.wit",
    world: "data-renderer-plugin",
});

/// Fuel budget for one render — bounded like the exporter so a runaway renderer
/// traps instead of hanging the UI (this runs on the paint path).
const RENDER_FUEL: u64 = 50_000_000_000;

struct RendererState {
    wasi: WasiCtx,
    table: ResourceTable,
    /// The one dataset this invocation may read.
    approved: String,
    /// Pins acquired by this invocation and not yet released by the guest.
    pins: Vec<String>,
}

impl RendererState {
    fn check_handle(
        &self,
        handle: &str,
    ) -> std::result::Result<(), thoth::plugin::dataset_read::PluginError> {
        if handle == self.approved {
            Ok(())
        } else {
            Err(dataset_read_error(format!(
                "dataset '{handle}' is not the one being rendered"
            )))
        }
    }

    /// Release pins the guest still holds. Called on every exit path so a
    /// guest that forgets to unpin (or traps) cannot keep a dataset resident.
    fn release_pins(&mut self) {
        for handle in std::mem::take(&mut self.pins) {
            crate::papyrus::unpin(&handle);
        }
    }
}

impl wasmtime_wasi::WasiView for RendererState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// Instantiate the renderer at `wasm_path` and run it over `handle`, returning
/// the serialized `RenderNode` tree (node-json) it produced.
pub fn run_render(engine: &Engine, wasm_path: &Path, handle: &str) -> Result<String> {
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
            approved: handle.to_string(),
            pins: Vec::new(),
        },
    );
    store
        .set_fuel(RENDER_FUEL)
        .map_err(|e| load_err(e.to_string()))?;

    let component = Component::from_file(engine, wasm_path).map_err(|e| load_err(e.to_string()))?;

    let mut linker = Linker::<RendererState>::new(engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| load_err(e.to_string()))?;
    thoth::plugin::dataset_read::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)
        .map_err(|e| load_err(e.to_string()))?;

    let bindings = DataRendererPlugin::instantiate(&mut store, &component, &linker).map_err(|e| {
        load_err(format!(
            "{e}\nIf this plugin was built for an older Thoth plugin API, rebuild it (`cargo component build --release`) or update it."
        ))
    })?;

    let call = bindings
        .thoth_plugin_data_renderer()
        .call_render(&mut store, handle);
    // Release any pins the guest still holds, whether it returned normally,
    // errored, or trapped.
    store.data_mut().release_pins();
    call.map_err(|e| ThothError::Unknown {
        message: e.to_string(),
    })?
    .map_err(|e| ThothError::Unknown { message: e.message })
}

fn dataset_read_error(message: impl Into<String>) -> thoth::plugin::dataset_read::PluginError {
    thoth::plugin::dataset_read::PluginError {
        code: 1,
        message: message.into(),
    }
}

impl thoth::plugin::dataset_read::Host for RendererState {
    fn info(
        &mut self,
        handle: String,
    ) -> std::result::Result<
        thoth::plugin::dataset_read::DatasetInfo,
        thoth::plugin::dataset_read::PluginError,
    > {
        self.check_handle(&handle)?;
        let meta = crate::papyrus::meta(&handle)
            .ok_or_else(|| dataset_read_error(format!("unknown dataset '{handle}'")))?;
        Ok(thoth::plugin::dataset_read::DatasetInfo {
            name: meta.name,
            kind: meta.kind,
            columns: meta
                .columns
                .into_iter()
                .map(|c| thoth::plugin::dataset_read::DatasetColumn {
                    name: c.name,
                    type_hint: c.type_hint,
                })
                .collect(),
            row_count: meta.row_count,
            revision: meta.revision,
        })
    }

    fn read(
        &mut self,
        handle: String,
        offset: u64,
        limit: u32,
    ) -> std::result::Result<
        thoth::plugin::dataset_read::DatasetPage,
        thoth::plugin::dataset_read::PluginError,
    > {
        self.check_handle(&handle)?;
        let page = crate::papyrus::read(&handle, offset, limit)
            .ok_or_else(|| dataset_read_error(format!("unknown dataset '{handle}'")))?;
        Ok(thoth::plugin::dataset_read::DatasetPage {
            columns: page
                .columns
                .into_iter()
                .map(|c| thoth::plugin::dataset_read::DatasetColumn {
                    name: c.name,
                    type_hint: c.type_hint,
                })
                .collect(),
            rows: page.rows,
            nulls: page.nulls,
            offset: page.offset,
            total: page.total,
        })
    }

    fn pin(&mut self, handle: String) -> bool {
        if handle != self.approved {
            return false;
        }
        if crate::papyrus::pin(&handle) {
            self.pins.push(handle);
            true
        } else {
            false
        }
    }

    fn unpin(&mut self, handle: String) -> bool {
        // Only release a pin this invocation actually acquired; a guest must
        // not be able to drop a chart's or another store's pin.
        let Some(pos) = self.pins.iter().position(|h| h == &handle) else {
            return false;
        };
        self.pins.remove(pos);
        crate::papyrus::unpin(&handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_approved_handle_is_readable() {
        let state = RendererState {
            wasi: WasiCtxBuilder::new().build(),
            table: ResourceTable::new(),
            approved: "ds-1".to_string(),
            pins: Vec::new(),
        };
        assert!(state.check_handle("ds-1").is_ok());
        assert!(state.check_handle("ds-2").is_err());
    }
}
