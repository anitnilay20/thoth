//! Shared abstraction over plugin loaders that render an interactive UI inside a
//! dock tab.
//!
//! Both [`WasmDataSourceLoader`](crate::plugin::wasm_data_source::WasmDataSourceLoader)
//! and [`WasmUiComponentLoader`](crate::plugin::wasm_ui_component::WasmUiComponentLoader)
//! implement [`PluginUiHost`], so an [`ActivePluginPane`](crate::state::ActivePluginPane)
//! can hold either behind a `Box<dyn PluginUiHost>` and the app's poll/dispatch loop
//! can drive both uniformly.
//!
//! The tab-state / lifecycle methods (`tab_title`, `get_state`, `on_tab_*`, …) map to
//! the optional `tab-host` WIT export. Loaders whose world does not export it (e.g. the
//! current data-source world) inherit the no-op defaults here.

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

/// Common interface for plugin loaders rendered inside a dock tab.
pub trait PluginUiHost: Send {
    fn plugin_id(&self) -> &str;

    fn render_ui(&self) -> Result<UiOutput>;
    fn handle_event(&self, event: UiEvent) -> Result<UiOutput>;
    fn render_sidebar(&self) -> Result<Option<UiOutput>>;

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
}
