//! Shared abstractions over live plugin instances.
//!
//! Both [`WasmDataSourceLoader`](crate::plugin::wasm_data_source::WasmDataSourceLoader)
//! and [`WasmUiComponentLoader`](crate::plugin::wasm_ui_component::WasmUiComponentLoader)
//! implement [`PluginCore`] and [`PluginUi`]. Runtime owners keep the core facet
//! behind a `Box<dyn PluginCore>` so headless code can drive lifecycle, async I/O,
//! and datasets without depending on rendering. GUI paths explicitly request the
//! optional [`PluginUi`] facet when they need to render or dispatch a widget event.
//!
//! The tab-state / lifecycle methods (`tab_title`, `get_state`, `on_tab_*`, …) map to
//! the `tab-host` WIT export, which both the `ui-component-plugin` and
//! `data-source-plugin` worlds export. The trait provides no-op defaults so any
//! future loader whose world omits `tab-host` still compiles; the two current
//! loaders override them to call the export.

use crate::error::Result;
use crate::plugin::render_node::{UiEvent, UiOutput};
use crate::settings::PluginSettingData;

/// Raw HTTP response — plain Send-safe types, no WIT bindgen involvement.
pub struct HttpResponseRaw {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub duration_ms: u64,
}

/// Result type for async HTTP. Uses `std::result::Result` explicitly to avoid
/// clashing with the crate-level `Result<T> = Result<T, ThothError>` alias.
pub type HttpCallResult = std::result::Result<HttpResponseRaw, String>;

/// A loader-agnostic HTTP request, so the trait does not depend on a concrete
/// loader's bindgen-generated `HttpRequest` type. Loaders convert to/from their
/// own WIT type at the boundary.
#[derive(Clone)]
pub struct PluginHttpRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

/// Upper bound on a plugin-supplied tab title (chars beyond this are dropped).
pub const MAX_TAB_TITLE_LEN: usize = 200;
/// Upper bound on a plugin-supplied seed-state blob; oversized blobs are dropped
/// (not truncated, which would corrupt the JSON).
pub const MAX_TAB_STATE_LEN: usize = 1 << 20; // 1 MiB

/// A plugin's request (via the `ui-tabs` host import) to open a new dock tab
/// hosting a fresh instance of itself.
#[derive(Clone, Debug)]
pub struct TabOpenRequest {
    /// Host-assigned id returned to the plugin from `open-tab`.
    pub request_id: String,
    /// The plugin that asked to open the tab — the new tab hosts the same plugin.
    pub plugin_id: String,
    pub title: String,
    pub icon: Option<String>,
    /// JSON blob to seed the new instance with via `init-with-state`.
    pub initial_state: Option<String>,
}

impl TabOpenRequest {
    /// Build a request with bounded title/state so a plugin can't push arbitrarily
    /// large payloads through the `open-tab` import.
    pub fn sanitized(
        request_id: String,
        plugin_id: String,
        mut title: String,
        icon: Option<String>,
        initial_state: Option<String>,
    ) -> Self {
        if title.len() > MAX_TAB_TITLE_LEN {
            let mut end = MAX_TAB_TITLE_LEN;
            while end > 0 && !title.is_char_boundary(end) {
                end -= 1;
            }
            title.truncate(end);
        }
        // Truncating a JSON blob would corrupt it, so drop it when oversized.
        let initial_state = initial_state.filter(|s| s.len() <= MAX_TAB_STATE_LEN);
        Self {
            request_id,
            plugin_id,
            title,
            icon,
            initial_state,
        }
    }
}

/// Headless-safe interface shared by every live plugin instance.
///
/// This trait deliberately contains no egui types or render callbacks. It is
/// object-safe so the host's core runtime can store heterogeneous plugins as
/// `Box<dyn PluginCore>`.
pub trait PluginCore: Send {
    fn plugin_id(&self) -> &str;

    /// Return the optional UI facet implemented by this plugin instance.
    /// Headless-only plugins keep the default `None` implementation.
    fn as_ui(&self) -> Option<&dyn PluginUi> {
        None
    }

    /// Unique id for this plugin *instance* (pane). Defaults to `plugin_id` for
    /// loaders that don't need per-instance identity; data-source loaders
    /// override it so two tabs of the same plugin keep separate status signals.
    fn instance_id(&self) -> &str {
        self.plugin_id()
    }

    /// True when the plugin's Store is currently held by a background worker (a
    /// blocking DB query is running). Callers use this to defer work that would
    /// otherwise block the UI thread on the Store mutex. Default: never busy.
    fn busy(&self) -> bool {
        false
    }

    /// Notify the plugin that its user-configured settings changed.
    fn on_setting_change(&self, settings: &[PluginSettingData]) -> Result<()> {
        let _ = settings;
        Ok(())
    }

    // ── tab integration (tab-host export; defaults for loaders without it) ──────

    /// Plugin-provided tab title. `None` → caller falls back to the plugin id.
    fn tab_title(&self) -> Option<String> {
        None
    }
    /// Plugin-provided Phosphor glyph for the tab label.
    fn tab_icon(&self) -> Option<String> {
        None
    }
    /// Serialize per-tab state for persistence. `None` when unsupported.
    fn get_state(&self) -> Result<Option<String>> {
        Ok(None)
    }
    /// Restore per-tab state from a previously saved blob.
    fn init_with_state(&self, _state: &str) -> Result<()> {
        Ok(())
    }
    fn on_tab_focused(&self) {}
    fn on_tab_blurred(&self) {}
    fn on_tab_closed(&self) {}

    /// Drain tab-open requests the plugin raised via the `ui-tabs` import.
    fn drain_tab_open_requests(&self) -> Vec<TabOpenRequest> {
        Vec::new()
    }

    // ── async HTTP (only data-source implements; defaults are no-ops) ───────────

    fn drain_http_results(&self) -> Vec<(String, HttpCallResult)> {
        Vec::new()
    }
    fn drain_retry_requests(&self) -> Vec<(String, PluginHttpRequest)> {
        Vec::new()
    }
    fn dispatch_approved_request(&self, _request_id: String, _req: PluginHttpRequest) {}
    fn has_pending_http(&self) -> bool {
        false
    }

    // ── async DB queries (only data-source implements; defaults are no-ops) ──────

    /// Drain queued `submit-query` requests and run each on a worker thread.
    fn pump_queries(&self) {}
    /// Drain completed query results: `(request_id, Ok(rows-json) | Err(message))`.
    fn drain_query_results(&self) -> Vec<(String, std::result::Result<String, String>)> {
        Vec::new()
    }
    fn has_pending_query(&self) -> bool {
        false
    }

    // ── websocket events (only data-source implements; default is a no-op) ───────

    /// Drain WebSocket lifecycle + message events: `(connection_id, event)`.
    fn drain_ws_events(&self) -> Vec<(String, crate::plugin::websocket::WsEvent)> {
        Vec::new()
    }

    // ── data-producer (dataset bus) ──────────────────────────────────────────────

    /// Whether this plugin can produce a dataset (exports `data-producer`).
    fn is_data_producer(&self) -> bool {
        false
    }

    /// Fetch this producer's current dataset (host calls the `provide-dataset`
    /// export on demand). Default: not a producer.
    fn provide_dataset(&self) -> Result<ProvidedDataset> {
        Err(crate::error::ThothError::Unknown {
            message: "plugin is not a data producer".to_string(),
        })
    }
}

/// Rendering facet for plugins that expose an interactive UI.
///
/// A CLI-only plugin implements [`PluginCore`] without implementing this trait.
/// The host obtains this facet through [`PluginCore::as_ui`] only in GUI paths.
pub trait PluginUi: PluginCore {
    fn render_ui(&self) -> Result<UiOutput>;
    fn handle_event(&self, event: UiEvent) -> Result<UiOutput>;
    fn render_sidebar(&self) -> Result<Option<UiOutput>>;
}

/// A dataset returned by a producer's `provide-dataset` export, in plain
/// host-side types (no bindgen dependency) so it can cross into the bus.
pub struct ProvidedDataset {
    pub name: String,
    pub kind: String,
    /// `(column name, SQL-ish type hint)` pairs.
    pub columns: Vec<(String, String)>,
    pub rows: Vec<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::PluginCore;

    struct HeadlessPlugin;

    impl PluginCore for HeadlessPlugin {
        fn plugin_id(&self) -> &str {
            "test.headless"
        }
    }

    #[test]
    fn core_is_object_safe_without_a_ui_facet() {
        let plugin: Box<dyn PluginCore> = Box::new(HeadlessPlugin);
        assert_eq!(plugin.plugin_id(), "test.headless");
        assert!(plugin.as_ui().is_none());
    }
}
