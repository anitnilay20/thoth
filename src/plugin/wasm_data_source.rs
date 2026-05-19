use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::app::persistent_state::PersistentState;
use crate::consent::manager::{ConsentCallback, ConsentManager};
use crate::error::{Result, ThothError};
use crate::plugin::network_policy::{CheckOutcome, NetworkPolicy};
use crate::settings::PluginSettingData;

/// Fuel budget per WASM call.
/// serde_json serializing a large HTTP response body through nested Value calls
/// can consume hundreds of millions of WASM instructions, so the budget must be
/// high enough to handle realistic payloads while still bounding runaway plugins.
const PLUGIN_FUEL_BUDGET: u64 = 5_000_000_000;

/// Refuel the store to `PLUGIN_FUEL_BUDGET` before each WASM call.
fn refuel(store: &mut Store<DataSourcePluginState>) -> Result<()> {
    store
        .set_fuel(PLUGIN_FUEL_BUDGET)
        .map_err(|e| ThothError::Unknown {
            message: format!("failed to set plugin fuel: {e}"),
        })
}

wasmtime::component::bindgen!({
    path: "wit/thoth-plugin.wit",
    world: "data-source-plugin",
});

// Convenient re-exports so callers don't depend on the bindgen internals.
pub use exports::thoth::plugin::data_source::{ConfigEntry, PluginError, SourceSchema};

use crate::plugin::render_node::{UiEvent, UiNode, UiOutput};

// ── consent request ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ConsentRequest {
    pub plugin_id: String,
    pub domain: String,
}

// ── async HTTP result sent through the mpsc channel ──────────────────────────

/// Raw HTTP response — plain Send-safe types, no WIT bindgen involvement.
pub struct HttpResponseRaw {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub duration_ms: u64,
}

/// Result type for async HTTP — uses std::result::Result explicitly to avoid
/// conflicting with the crate-level `type Result<T> = Result<T, ThothError>` alias.
pub type HttpCallResult = std::result::Result<HttpResponseRaw, String>;

// ── atomic counter so callers can know when requests are in flight ────────────

static REQUEST_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_request_id() -> String {
    format!(
        "req-{}",
        REQUEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

// ── per-store state ───────────────────────────────────────────────────────────

struct DataSourcePluginState {
    wasi: WasiCtx,
    table: ResourceTable,
    policy: NetworkPolicy,
    plugin_id: String,
    consent_tx: std::sync::mpsc::Sender<ConsentRequest>,
    // Channel used by submit() to deliver async HTTP results back to the host.
    http_tx: std::sync::mpsc::Sender<(String, HttpCallResult)>,
    // Counts requests that have been submitted but not yet drained.
    // Shared with WasmDataSourceLoader so it can call request_repaint while waiting.
    pending_count: Arc<AtomicUsize>,
    // Retry channel: consent-approved requests are sent here so the host can
    // re-dispatch them on a background thread without needing self to be Sync.
    retry_tx:
        Arc<Mutex<std::sync::mpsc::Sender<(String, thoth::plugin::http_client::HttpRequest)>>>,
}

impl WasiView for DataSourcePluginState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

// ── consent helper ────────────────────────────────────────────────────────────

impl DataSourcePluginState {
    /// Register a pending consent request for `domain`.
    ///
    /// Notifies the host via `consent_tx` so the UI can show an indicator, then
    /// queues a `ConsentManager` entry whose `on_allow` closure retries `req`
    /// through `retry_tx` (the host re-dispatches it as a normal async request).
    /// `on_deny` is called if the user rejects; for synchronous `fetch()` callers
    /// the plugin already received an error, so pass `Arc::new(|| {})`.
    fn push_consent(
        &self,
        domain: &str,
        req: thoth::plugin::http_client::HttpRequest,
        retry_id: String,
        on_deny: ConsentCallback,
    ) {
        let _ = self.consent_tx.send(ConsentRequest {
            domain: domain.to_string(),
            plugin_id: self.plugin_id.clone(),
        });
        let retry_tx = Arc::clone(&self.retry_tx);
        let runtime_allowed = self.policy.runtime_allowed_handle();
        let dom = domain.to_string();
        ConsentManager::push_http_consent(
            domain,
            &self.plugin_id,
            Arc::new(move |remember: bool| {
                if let Ok(tx) = retry_tx.lock() {
                    let _ = tx.send((retry_id.clone(), req.clone()));
                }
                if remember
                    && let Ok(mut list) = runtime_allowed.lock() {
                        list.push(dom.clone());
                    }
            }),
            on_deny,
        );
    }
}

// ── http-client WIT import — host side ───────────────────────────────────────

impl thoth::plugin::http_client::Host for DataSourcePluginState {
    /// Synchronous fetch — used by data_source::query / schema programmatic paths.
    fn fetch(
        &mut self,
        req: thoth::plugin::http_client::HttpRequest,
    ) -> std::result::Result<
        thoth::plugin::http_client::HttpResponse,
        thoth::plugin::http_client::PluginError,
    > {
        match self.policy.check(&req.url) {
            Ok(CheckOutcome::Allowed) => {
                execute_http_request(req).map_err(|e| thoth::plugin::http_client::PluginError {
                    code: 1,
                    message: e,
                })
            }
            Ok(CheckOutcome::NeedsConsent { domain }) => {
                // fetch() is synchronous — return Err immediately.
                // Show the consent modal so the user is informed, and update
                // runtime_allowed on approval so a subsequent fetch() succeeds.
                // We do NOT queue a retry here: the plugin already received an
                // error and there is no request-id for the caller to correlate
                // an async result against.
                let _ = self.consent_tx.send(ConsentRequest {
                    domain: domain.clone(),
                    plugin_id: self.plugin_id.clone(),
                });
                let runtime_allowed = self.policy.runtime_allowed_handle();
                let dom = domain.clone();
                ConsentManager::push_http_consent(
                    &domain,
                    &self.plugin_id,
                    Arc::new(move |remember: bool| {
                        if remember
                            && let Ok(mut list) = runtime_allowed.lock() {
                                list.push(dom.clone());
                            }
                    }),
                    Arc::new(|_| {}),
                );
                Err(thoth::plugin::http_client::PluginError {
                    code: 403,
                    message: format!("domain '{domain}' not approved — waiting for user consent"),
                })
            }
            Err(violation) => Err(thoth::plugin::http_client::PluginError {
                code: 403,
                message: format!("blocked: {violation:?}"),
            }),
        }
    }

    /// Async submit — returns a request_id immediately; the host delivers the
    /// result back to the plugin via handle_event(widget_id=request_id,
    /// kind="http-response", value=<json>).
    fn submit(&mut self, req: thoth::plugin::http_client::HttpRequest) -> String {
        let request_id = next_request_id();
        let tx = self.http_tx.clone();
        let id = request_id.clone();

        match self.policy.check(&req.url) {
            Ok(CheckOutcome::Allowed) => {
                // Increment before spawning so has_pending_http() is true
                // on the very next poll.
                self.pending_count.fetch_add(1, Ordering::Relaxed);
                std::thread::spawn(move || {
                    let start = std::time::Instant::now();
                    let outcome = execute_http_request(req).map(|r| HttpResponseRaw {
                        status: r.status,
                        headers: r.headers,
                        body: r.body,
                        duration_ms: start.elapsed().as_millis() as u64,
                    });
                    let _ = tx.send((id, outcome));
                });
            }
            Ok(CheckOutcome::NeedsConsent { domain }) => {
                let deny_tx = tx.clone();
                let deny_id = id.clone();
                let deny_domain = domain.clone();
                let deny_count = Arc::clone(&self.pending_count);
                self.push_consent(
                    &domain,
                    req,
                    request_id.clone(),
                    Arc::new(move |_remember: bool| {
                        deny_count.fetch_add(1, Ordering::Relaxed);
                        let _ = deny_tx.send((
                            deny_id.clone(),
                            Err::<HttpResponseRaw, String>(format!(
                                "domain '{deny_domain}' access denied by user"
                            )),
                        ));
                    }),
                );

                // Immediate "awaiting consent" notification — lets the plugin update
                // its UI state while the modal is open.
                self.pending_count.fetch_add(1, Ordering::Relaxed);
                let _ = tx.send((
                    id,
                    Err::<HttpResponseRaw, String>(format!(
                        "domain '{domain}' not approved — waiting for user consent"
                    )),
                ));
            }
            Err(violation) => {
                self.pending_count.fetch_add(1, Ordering::Relaxed);
                let _ = tx.send((
                    id,
                    Err::<HttpResponseRaw, String>(format!("blocked: {violation:?}")),
                ));
            }
        }

        request_id
    }
}

impl thoth::plugin::plugin_storage::Host for DataSourcePluginState {
    fn read(&mut self) -> String {
        let path = match PersistentState::plugin_state_path(&self.plugin_id) {
            Ok(p) => p,
            Err(_) => return String::new(),
        };
        std::fs::read_to_string(&path).unwrap_or_default()
    }

    fn write(&mut self, data: String) -> std::result::Result<(), String> {
        let path =
            PersistentState::plugin_state_path(&self.plugin_id).map_err(|err| err.to_string())?;
        std::fs::write(&path, data.as_bytes()).map_err(|e| e.to_string())
    }
}

// ── reqwest bridge ────────────────────────────────────────────────────────────

fn execute_http_request(
    req: thoth::plugin::http_client::HttpRequest,
) -> std::result::Result<thoth::plugin::http_client::HttpResponse, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let mut builder = match req.method.to_uppercase().as_str() {
        "POST" => client.post(&req.url),
        "PUT" => client.put(&req.url),
        "PATCH" => client.patch(&req.url),
        "DELETE" => client.delete(&req.url),
        _ => client.get(&req.url),
    };
    for (k, v) in &req.headers {
        builder = builder.header(k.as_str(), v.as_str());
    }
    if let Some(body) = req.body {
        builder = builder.body(body);
    }

    let resp = builder.send().map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let headers = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let body = resp.bytes().map_err(|e| e.to_string())?.to_vec();

    Ok(thoth::plugin::http_client::HttpResponse {
        status,
        headers,
        body,
    })
}

// ── inner / outer structs ─────────────────────────────────────────────────────

struct WasmDataSourceInner {
    store: Store<DataSourcePluginState>,
    bindings: DataSourcePlugin,
}

pub struct WasmDataSourceLoader {
    inner: Mutex<WasmDataSourceInner>,
    consent_rx: std::sync::mpsc::Receiver<ConsentRequest>,
    /// Receives completed async HTTP results submitted via `submit()`.
    http_rx: std::sync::mpsc::Receiver<(String, HttpCallResult)>,
    /// Receives retry requests from consent-approved callbacks.
    retry_rx: std::sync::mpsc::Receiver<(String, thoth::plugin::http_client::HttpRequest)>,
    /// Number of submitted requests that haven't been drained yet.
    pending_count: Arc<AtomicUsize>,
    plugin_id: String,
}

// ── public API ────────────────────────────────────────────────────────────────

impl WasmDataSourceLoader {
    /// Load and instantiate a data-source WASM plugin.
    ///
    /// * `policy`    — per-plugin network policy (built from plugin.toml + user settings)
    /// * `plugin_id` — used to tag any consent requests raised during http calls
    pub fn open(
        engine: &Engine,
        wasm_path: &Path,
        policy: NetworkPolicy,
        plugin_id: String,
        settings: &[PluginSettingData],
    ) -> Result<Self> {
        let wasi = WasiCtxBuilder::new().inherit_stdio().build();

        let (consent_tx, consent_rx) = std::sync::mpsc::channel::<ConsentRequest>();
        let (http_tx, http_rx) = std::sync::mpsc::channel::<(String, HttpCallResult)>();
        let (retry_tx, retry_rx) =
            std::sync::mpsc::channel::<(String, thoth::plugin::http_client::HttpRequest)>();
        let pending_count = Arc::new(AtomicUsize::new(0));
        let retry_tx_shared = Arc::new(Mutex::new(retry_tx));

        let state = DataSourcePluginState {
            wasi,
            table: ResourceTable::new(),
            policy,
            plugin_id: plugin_id.clone(),
            consent_tx,
            http_tx,
            pending_count: Arc::clone(&pending_count),
            retry_tx: Arc::clone(&retry_tx_shared),
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

        let mut linker = Linker::<DataSourcePluginState>::new(engine);

        // 1. Register all WASI imports (stdio, clocks, random, …).
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| {
            ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            }
        })?;

        // 2. Register the http-client import — wired to our Host impl above.
        thoth::plugin::http_client::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s).map_err(
            |e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            },
        )?;

        // 3. Register the plugin-storage import.
        thoth::plugin::plugin_storage::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s).map_err(
            |e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            },
        )?;

        let bindings =
            DataSourcePlugin::instantiate(&mut store, &component, &linker).map_err(|e| {
                ThothError::PluginLoadError {
                    path: wasm_path.to_path_buf(),
                    reason: e.to_string(),
                }
            })?;

        let mut loader = Self {
            inner: Mutex::new(WasmDataSourceInner { store, bindings }),
            consent_rx,
            http_rx,
            retry_rx,
            pending_count,
            plugin_id,
        };

        // Always call on_load so the plugin can initialise from its own
        // persistent storage even when there are no user-configured settings.
        loader.on_load(settings)?;

        Ok(loader)
    }

    /// Invoke the plugin's on-load lifecycle hook with the provided settings.
    /// Settings are serialized as a JSON array of `{key, value}` objects.
    pub fn on_load(&mut self, settings: &[PluginSettingData]) -> Result<()> {
        let settings_json = serde_json::to_string(settings).map_err(|e| ThothError::Unknown {
            message: format!("Failed to serialize plugin settings: {e}"),
        })?;

        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        refuel(store)?;
        bindings
            .thoth_plugin_plugin_lifecycle()
            .call_on_load(store, &settings_json)
            .map_err(|e| ThothError::PluginLoadError {
                path: std::path::Path::new("<plugin on_load>").to_path_buf(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    /// Invoke the plugin's on-setting-change lifecycle hook with the updated settings.
    /// Settings are serialized as a JSON array of `{key, value}` objects.
    pub fn on_setting_change(&mut self, settings: &[PluginSettingData]) -> Result<()> {
        let settings_json = serde_json::to_string(settings).map_err(|e| ThothError::Unknown {
            message: format!("Failed to serialize plugin settings: {e}"),
        })?;

        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        refuel(store)?;
        bindings
            .thoth_plugin_plugin_lifecycle()
            .call_on_setting_change(store, &settings_json)
            .map_err(|e| ThothError::PluginLoadError {
                path: std::path::Path::new("<plugin on_setting_change>").to_path_buf(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    /// Configuration fields the plugin needs — the host form renders these.
    pub fn required_config(&self) -> Result<Vec<ConfigEntry>> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        refuel(store)?;
        bindings
            .thoth_plugin_data_source()
            .call_required_config(store)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })
    }

    /// Establish a connection with the provided config values.
    pub fn connect(&self, config: Vec<ConfigEntry>) -> Result<String> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        refuel(store)?;
        bindings
            .thoth_plugin_data_source()
            .call_connect(store, &config)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })
    }

    /// Infer schema from the connected source.
    pub fn schema(&self, handle: &str) -> Result<Vec<SourceSchema>> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        refuel(store)?;
        bindings
            .thoth_plugin_data_source()
            .call_schema(store, handle)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })
    }

    /// Execute a query against the connected source.
    pub fn query(&self, handle: &str, q: &str) -> Result<String> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        refuel(store)?;
        bindings
            .thoth_plugin_data_source()
            .call_query(store, handle, q)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })
    }

    /// Release the connection.
    pub fn close(&self, handle: &str) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        let _ = refuel(store);
        let _ = bindings
            .thoth_plugin_data_source()
            .call_close(store, handle);
    }

    /// Render the active query result as a `RenderNode` tree for the main pane.
    pub fn render_pane(&self, handle: &str) -> crate::error::Result<UiNode> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        refuel(store)?;
        let wit_out = bindings
            .thoth_plugin_data_source()
            .call_render_pane(store, handle)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })?;
        serde_json::from_str(&wit_out.node_json).map_err(|e| ThothError::Unknown {
            message: format!("pane RenderNode parse error: {e}"),
        })
    }

    /// Ask the plugin if it wants to render a sidebar panel.
    /// Returns `None` when the plugin has no sidebar content.
    pub fn render_sidebar(&self) -> Result<Option<UiOutput>> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
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

    /// Ask the plugin to render its initial UI tree.
    pub fn render_ui(&self) -> Result<UiOutput> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
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

    /// Forward a widget interaction to the plugin and get a fresh UI tree back.
    pub fn handle_event(&self, event: UiEvent) -> Result<UiOutput> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
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

    /// Non-blocking drain of consent requests raised during the last WASM call.
    pub fn drain_consent_requests(&self) -> Vec<ConsentRequest> {
        let mut out = Vec::new();
        while let Ok(req) = self.consent_rx.try_recv() {
            out.push(req);
        }
        out
    }

    /// Non-blocking drain of completed async HTTP results.
    /// Each entry is `(request_id, outcome)` — call `handle_event` for each.
    pub fn drain_http_results(&self) -> Vec<(String, HttpCallResult)> {
        let mut out = Vec::new();
        while let Ok(result) = self.http_rx.try_recv() {
            self.pending_count.fetch_sub(1, Ordering::Relaxed);
            out.push(result);
        }
        out
    }

    /// Non-blocking drain of retry requests enqueued by consent callbacks.
    /// Each entry is `(original_request_id, request)`. The host should re-dispatch
    /// these on a background thread (bypassing the policy check since user approved)
    /// and deliver the result via `handle_event` using the original request_id.
    pub fn drain_retry_requests(&self) -> Vec<(String, thoth::plugin::http_client::HttpRequest)> {
        let mut out = Vec::new();
        while let Ok(item) = self.retry_rx.try_recv() {
            out.push(item);
        }
        out
    }

    /// Re-dispatch a consent-approved request on a background thread.
    /// Increments pending_count and delivers the result to `http_rx` as usual,
    /// so `drain_http_results` + `handle_event` pick it up on the next poll.
    pub fn dispatch_approved_request(
        &self,
        request_id: String,
        req: thoth::plugin::http_client::HttpRequest,
    ) {
        let tx = {
            let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            guard.store.data().http_tx.clone()
        };
        self.pending_count.fetch_add(1, Ordering::Relaxed);
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let outcome = execute_http_request(req).map(|r| HttpResponseRaw {
                status: r.status,
                headers: r.headers,
                body: r.body,
                duration_ms: start.elapsed().as_millis() as u64,
            });
            let _ = tx.send((request_id, outcome));
        });
    }

    /// True while at least one `submit()` request is still in flight.
    /// The host should call `ctx.request_repaint()` while this returns true.
    pub fn has_pending_http(&self) -> bool {
        self.pending_count.load(Ordering::Relaxed) > 0
    }

    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }
}
