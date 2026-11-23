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
    /// The one dataset this invocation may read.
    approved: String,
    /// Pins acquired by this invocation and not yet released by the guest.
    pins: Vec<String>,
}

impl ExporterState {
    fn check_handle(
        &self,
        handle: &str,
    ) -> std::result::Result<(), thoth::plugin::dataset_read::PluginError> {
        if handle == self.approved {
            Ok(())
        } else {
            Err(dataset_read_error(format!(
                "dataset '{handle}' is not the one being exported"
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
            approved: handle.to_string(),
            pins: Vec::new(),
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

    let call = bindings
        .thoth_plugin_exporter()
        .call_run(&mut store, handle, options);
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

impl thoth::plugin::dataset_read::Host for ExporterState {
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

    fn state(approved: &str) -> ExporterState {
        ExporterState {
            wasi: WasiCtxBuilder::new().build(),
            table: ResourceTable::new(),
            approved: approved.to_string(),
            pins: Vec::new(),
        }
    }

    #[test]
    fn only_the_approved_handle_is_readable() {
        let state = state("ds-1");
        assert!(state.check_handle("ds-1").is_ok());
        assert!(state.check_handle("ds-2").is_err());
    }

    #[test]
    fn unpin_only_releases_pins_this_invocation_acquired() {
        let _g = crate::papyrus::tests::exclusive();
        let id = crate::papyrus::publish(
            "test",
            "test#pins",
            "d".into(),
            "k".into(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let mut state = state(&id);
        // A chart (or another store) owns a pin; this invocation must not drop it.
        assert!(crate::papyrus::pin(&id));
        assert!(!thoth::plugin::dataset_read::Host::unpin(
            &mut state,
            id.clone()
        ));
        // This invocation acquires and releases its own pin.
        assert!(thoth::plugin::dataset_read::Host::pin(
            &mut state,
            id.clone()
        ));
        assert_eq!(state.pins, vec![id.clone()]);
        assert!(thoth::plugin::dataset_read::Host::unpin(
            &mut state,
            id.clone()
        ));
        assert!(state.pins.is_empty());
        assert!(!thoth::plugin::dataset_read::Host::unpin(
            &mut state,
            id.clone()
        ));
        assert!(crate::papyrus::unpin(&id));
        crate::papyrus::release("test#pins", &id);
    }
}
