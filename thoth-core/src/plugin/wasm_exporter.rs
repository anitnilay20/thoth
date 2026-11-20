//! On-demand runner for **exporter** plugins (#135).
//!
//! Exporters are stateless: the host owns the dataset, hands the plugin a
//! handle, and calls `exporter.run`; the plugin pages the rows through the
//! `dataset-read` import and returns the formatted file bytes, which the host
//! writes. Unlike the
//! stateful loaders, an exporter is instantiated fresh per invocation and
//! dropped afterwards — no per-instance state to keep.

use std::path::Path;

use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView};

use crate::error::{Result, ThothError};

wasmtime::component::bindgen!({
    path: "../wit/thoth-plugin.wit",
    world: "exporter-plugin",
});

/// Fuel budget for one export. Generous enough for a full (bounded) dataset yet
/// finite, so a runaway exporter traps rather than hanging the UI.
const EXPORT_FUEL: u64 = 50_000_000_000;

struct ExporterState {
    wasi: WasiCtx,
    table: ResourceTable,
}

impl wasmtime_wasi::WasiView for ExporterState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// Instantiate the exporter at `wasm_path` and run it over `handle`, returning
/// the formatted file bytes. `options` are the export options the plugin
/// declared (currently unused by the built-in CSV exporter).
pub fn run_export(
    engine: &Engine,
    wasm_path: &Path,
    handle: &str,
    options: &[(String, String)],
) -> Result<Vec<u8>> {
    let load_err = |e: String| ThothError::PluginLoadError {
        path: wasm_path.to_path_buf(),
        reason: e,
    };

    let wasi = WasiCtxBuilder::new().inherit_stdio().build();
    let mut store = Store::new(
        engine,
        ExporterState {
            wasi,
            table: ResourceTable::new(),
        },
    );
    // Formatting touches every row but the dataset is bounded (≤ MAX_STREAM_ROWS
    // rows, ≤ MAX_BYTES total), so a large-but-finite budget lets real exports
    // finish while a runaway plugin still traps instead of hanging the app.
    store
        .set_fuel(EXPORT_FUEL)
        .map_err(|e| load_err(e.to_string()))?;

    let component = Component::from_file(engine, wasm_path).map_err(|e| load_err(e.to_string()))?;

    let mut linker = Linker::<ExporterState>::new(engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| load_err(e.to_string()))?;
    thoth::plugin::dataset_read::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)
        .map_err(|e| load_err(e.to_string()))?;

    let bindings = ExporterPlugin::instantiate(&mut store, &component, &linker)
        .map_err(|e| load_err(e.to_string()))?;

    bindings
        .thoth_plugin_exporter()
        .call_run(&mut store, handle, options)
        .map_err(|e| ThothError::Unknown {
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

impl thoth::plugin::dataset_read::Host for ExporterState {
    fn info(
        &mut self,
        handle: String,
    ) -> std::result::Result<
        thoth::plugin::dataset_read::DatasetInfo,
        thoth::plugin::dataset_read::PluginError,
    > {
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
        crate::papyrus::pin(&handle)
    }

    fn unpin(&mut self, handle: String) -> bool {
        crate::papyrus::unpin(&handle)
    }
}
