use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

// These plain Send-safe types live in `plugin_ui_host` so they can be shared with
// the `PluginUiHost` trait without depending on this loader's bindgen internals.
pub use crate::plugin::plugin_ui_host::{HttpCallResult, HttpResponseRaw};
use crate::plugin::plugin_ui_host::{PluginHttpRequest, PluginUiHost, TabOpenRequest};

// ── atomic counter so callers can know when requests are in flight ────────────

static REQUEST_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_request_id() -> String {
    format!(
        "req-{}",
        REQUEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

fn next_tab_request_id() -> String {
    format!(
        "tab-{}",
        REQUEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

fn next_query_request_id() -> String {
    format!(
        "q-{}",
        REQUEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

/// A query the plugin asked the host to run async: (request-id, handle, sql).
type QueryRequest = (String, String, String);
/// Result of an async query: rows-JSON on success, message on failure.
pub type QueryResult = std::result::Result<String, String>;

// ── tcp-client (host-terminated TLS) ────────────────────────────────────────

/// Connect/read/write timeouts for the `tcp-client` import. A blocking read on a
/// hung connection consumes no fuel, so these bound it (see DATABASE_PLUGINS.md).
const TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const TCP_IO_TIMEOUT: Duration = Duration::from_secs(60);
/// Cap a single `read` allocation so a plugin can't request a huge buffer.
const TCP_READ_CAP: usize = 1 << 20; // 1 MiB

/// A plaintext or TLS-wrapped stream the plugin reads/writes through `tcp-client`.
/// TLS is terminated host-side so the plugin always sees decrypted bytes.
trait ReadWrite: Read + Write + Send {}
impl<T: Read + Write + Send> ReadWrite for T {}

/// Adapts an already-boxed stream back into a concrete `Read + Write` so it can
/// be handed to `tcp_tls` for an in-place STARTTLS upgrade.
struct BoxIo(Box<dyn ReadWrite>);
impl Read for BoxIo {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}
impl Write for BoxIo {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

/// Service name used for the `secure-storage` keychain entries.
#[cfg_attr(test, allow(dead_code))] // the test build uses an in-memory secret store
const KEYRING_SERVICE: &str = "com.thoth.app";

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
    // Tab-open requests raised by the plugin via the `ui-tabs` import.
    tab_tx: std::sync::mpsc::Sender<TabOpenRequest>,
    // Open TCP/TLS streams from the `tcp-client` import, keyed by an opaque id.
    tcp_streams: HashMap<u64, Box<dyn ReadWrite>>,
    next_tcp_id: u64,
    // Async query requests raised by the plugin via `db-runtime::submit-query`.
    query_request_tx: std::sync::mpsc::Sender<QueryRequest>,
    // The query currently executing on the worker thread, so a tcp-client connect
    // that hits the consent gate can re-enqueue it once the user approves the host.
    current_query: Option<QueryRequest>,
    // Result channel + in-flight counter (shared with the loader) so the consent
    // DENY path can deliver a terminal failure for the in-flight query instead of
    // leaving its UI waiting forever.
    query_result_tx: std::sync::mpsc::Sender<(String, QueryResult)>,
    query_pending: Arc<AtomicUsize>,
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
                if remember && let Ok(mut list) = runtime_allowed.lock() {
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
                        if remember && let Ok(mut list) = runtime_allowed.lock() {
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

impl thoth::plugin::ui_tabs::Host for DataSourcePluginState {
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

// ── tcp-client WIT import — host side (TLS terminated here) ──────────────────

fn tcp_err(code: u32, message: impl Into<String>) -> thoth::plugin::tcp_client::PluginError {
    thoth::plugin::tcp_client::PluginError {
        code,
        message: message.into(),
    }
}

/// Open a plaintext TCP stream with connect/IO timeouts.
fn tcp_connect(host: &str, port: u16) -> std::io::Result<TcpStream> {
    use std::net::ToSocketAddrs;
    let mut last_err = std::io::Error::other("no addresses resolved");
    for addr in (host, port).to_socket_addrs()? {
        match TcpStream::connect_timeout(&addr, TCP_CONNECT_TIMEOUT) {
            Ok(s) => {
                s.set_read_timeout(Some(TCP_IO_TIMEOUT)).ok();
                s.set_write_timeout(Some(TCP_IO_TIMEOUT)).ok();
                return Ok(s);
            }
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

/// Wrap any byte stream with host-side TLS (rustls + Mozilla roots). Generic so
/// it works both at connect time (a fresh `TcpStream`) and for an in-place
/// STARTTLS upgrade of an already-open plaintext stream.
fn tcp_tls<S: Read + Write + Send + 'static>(
    stream: S,
    host: &str,
) -> std::result::Result<Box<dyn ReadWrite>, String> {
    // The TLS toggle maps to libpq `sslmode=require`: encrypt the connection but
    // do not verify the server certificate's issuer/chain. Database GUIs do this
    // for a plain "use SSL" toggle so self-signed / internal-CA servers connect.
    // It encrypts but does not authenticate the server (a future verify-full
    // option would). Hostname is still required for SNI.
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyServerCert))
        .with_no_client_auth();
    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|e| format!("invalid TLS server name '{host}': {e}"))?;
    let conn = rustls::ClientConnection::new(Arc::new(config), server_name)
        .map_err(|e| format!("TLS setup failed: {e}"))?;
    Ok(Box::new(rustls::StreamOwned::new(conn, stream)))
}

/// A rustls verifier that accepts any server certificate (encryption without
/// authentication) — the `sslmode=require` posture. See [`tcp_tls`].
#[derive(Debug)]
struct AcceptAnyServerCert;

impl rustls::client::danger::ServerCertVerifier for AcceptAnyServerCert {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        use rustls::SignatureScheme::*;
        vec![
            RSA_PKCS1_SHA256,
            RSA_PKCS1_SHA384,
            RSA_PKCS1_SHA512,
            ECDSA_NISTP256_SHA256,
            ECDSA_NISTP384_SHA384,
            ECDSA_NISTP521_SHA512,
            RSA_PSS_SHA256,
            RSA_PSS_SHA384,
            RSA_PSS_SHA512,
            ED25519,
        ]
    }
}

impl thoth::plugin::tcp_client::Host for DataSourcePluginState {
    fn connect(
        &mut self,
        host: String,
        port: u16,
        tls: bool,
    ) -> std::result::Result<u64, thoth::plugin::tcp_client::PluginError> {
        // The user/connection-host is the gate: allowlist or per-host consent.
        // Unlike the HTTP SSRF guard we do NOT block private/loopback ranges —
        // database clients legitimately target localhost and internal networks.
        match self.policy.check_tcp(&host) {
            Ok(CheckOutcome::Allowed) => {}
            Ok(CheckOutcome::NeedsConsent { domain }) => {
                let _ = self.consent_tx.send(ConsentRequest {
                    domain: domain.clone(),
                    plugin_id: self.plugin_id.clone(),
                });
                let runtime_allowed = self.policy.runtime_allowed_handle();
                let dom = domain.clone();
                // On approval, allow the host AND re-run the query that triggered
                // this connect (same request id), so the user doesn't have to
                // press Run again. The retry's check_tcp now returns Allowed.
                let retry_query = self.current_query.clone();
                let query_tx = self.query_request_tx.clone();
                // For the deny path: fail the in-flight query (it's kept pending
                // for the approve-and-retry path) so its UI stops waiting.
                let deny_query = self.current_query.clone();
                let deny_result_tx = self.query_result_tx.clone();
                let deny_pending = Arc::clone(&self.query_pending);
                let deny_dom = domain.clone();
                ConsentManager::push_http_consent(
                    &domain,
                    &self.plugin_id,
                    Arc::new(move |_remember: bool| {
                        // Always allow the host for the rest of the session: a DB
                        // client opens a fresh connection per query, so unless the
                        // approval is recorded the re-run (and every later query)
                        // would just hit the consent gate again. Unlike the HTTP
                        // retry path, the re-enqueued query re-checks the policy.
                        if let Ok(mut list) = runtime_allowed.lock()
                            && !list.iter().any(|d| d == &dom)
                        {
                            list.push(dom.clone());
                        }
                        if let Some(q) = &retry_query {
                            let _ = query_tx.send(q.clone());
                        }
                    }),
                    Arc::new(move |_| {
                        // User declined the host. Deliver a terminal failure for
                        // the query that triggered consent so drain_query_results
                        // completes it (matching pump_queries' +1 / drain's -1
                        // accounting) instead of leaving the UI pending forever.
                        if let Some((req_id, _, _)) = &deny_query {
                            deny_pending.fetch_add(1, Ordering::Relaxed);
                            let _ = deny_result_tx.send((
                                req_id.clone(),
                                Err(format!("connection to '{deny_dom}' denied")),
                            ));
                        }
                    }),
                );
                return Err(tcp_err(
                    403,
                    format!("host '{domain}' not approved — waiting for user consent"),
                ));
            }
            Err(violation) => return Err(tcp_err(403, format!("blocked: {violation:?}"))),
        }

        let tcp =
            tcp_connect(&host, port).map_err(|e| tcp_err(1, format!("connect failed: {e}")))?;
        let stream: Box<dyn ReadWrite> = if tls {
            tcp_tls(tcp, &host).map_err(|e| tcp_err(2, e))?
        } else {
            Box::new(tcp)
        };
        let id = self.next_tcp_id;
        self.next_tcp_id += 1;
        self.tcp_streams.insert(id, stream);
        Ok(id)
    }

    fn read(
        &mut self,
        stream: u64,
        max: u32,
    ) -> std::result::Result<Vec<u8>, thoth::plugin::tcp_client::PluginError> {
        let s = self
            .tcp_streams
            .get_mut(&stream)
            .ok_or_else(|| tcp_err(4, "invalid stream id"))?;
        let cap = (max as usize).min(TCP_READ_CAP);
        let mut buf = vec![0u8; cap];
        let n = s.read(&mut buf).map_err(|e| tcp_err(2, e.to_string()))?;
        buf.truncate(n);
        Ok(buf)
    }

    fn write(
        &mut self,
        stream: u64,
        bytes: Vec<u8>,
    ) -> std::result::Result<u32, thoth::plugin::tcp_client::PluginError> {
        let s = self
            .tcp_streams
            .get_mut(&stream)
            .ok_or_else(|| tcp_err(4, "invalid stream id"))?;
        s.write_all(&bytes).map_err(|e| tcp_err(2, e.to_string()))?;
        s.flush().map_err(|e| tcp_err(2, e.to_string()))?;
        Ok(bytes.len() as u32)
    }

    fn start_tls(
        &mut self,
        stream: u64,
        host: String,
    ) -> std::result::Result<(), thoth::plugin::tcp_client::PluginError> {
        // Take the plaintext stream out and replace it (same id) with a TLS
        // wrapper around it — the protocol has already done its SSL request.
        let plain = self
            .tcp_streams
            .remove(&stream)
            .ok_or_else(|| tcp_err(4, "invalid stream id"))?;
        let upgraded = tcp_tls(BoxIo(plain), &host).map_err(|e| tcp_err(2, e))?;
        self.tcp_streams.insert(stream, upgraded);
        Ok(())
    }

    fn close(&mut self, stream: u64) {
        self.tcp_streams.remove(&stream); // drop closes the socket
    }
}

// ── secure-storage WIT import — OS keychain via keyring ─────────────────────

fn se_err(message: impl Into<String>) -> thoth::plugin::secure_storage::PluginError {
    thoth::plugin::secure_storage::PluginError {
        code: 1,
        message: message.into(),
    }
}

impl DataSourcePluginState {
    /// Namespace keychain keys by plugin id so plugins can't read each other's secrets.
    fn scoped_key(&self, key: &str) -> String {
        format!("{}:{}", self.plugin_id, key)
    }
}

impl thoth::plugin::secure_storage::Host for DataSourcePluginState {
    fn write(
        &mut self,
        key: String,
        secret: String,
    ) -> std::result::Result<(), thoth::plugin::secure_storage::PluginError> {
        secret_store::write(&self.scoped_key(&key), &secret).map_err(se_err)
    }

    fn read(
        &mut self,
        key: String,
    ) -> std::result::Result<Option<String>, thoth::plugin::secure_storage::PluginError> {
        secret_store::read(&self.scoped_key(&key)).map_err(se_err)
    }

    fn delete(
        &mut self,
        key: String,
    ) -> std::result::Result<(), thoth::plugin::secure_storage::PluginError> {
        secret_store::delete(&self.scoped_key(&key)).map_err(se_err)
    }
}

/// Secret backend: the OS keychain in normal builds, an in-process map under
/// `cfg(test)` — so unit tests never touch (or hang/prompt on) a real keychain,
/// which keeps them reliable in CI (no D-Bus secret-service, no macOS prompt).
#[cfg(not(test))]
mod secret_store {
    use super::KEYRING_SERVICE;

    pub(super) fn write(account: &str, secret: &str) -> Result<(), String> {
        keyring::Entry::new(KEYRING_SERVICE, account)
            .and_then(|e| e.set_password(secret))
            .map_err(|e| e.to_string())
    }

    pub(super) fn read(account: &str) -> Result<Option<String>, String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, account).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub(super) fn delete(account: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, account).map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

#[cfg(test)]
mod secret_store {
    use std::collections::HashMap;
    use std::sync::{LazyLock, Mutex};

    static STORE: LazyLock<Mutex<HashMap<String, String>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    pub(super) fn write(account: &str, secret: &str) -> Result<(), String> {
        STORE
            .lock()
            .unwrap()
            .insert(account.to_string(), secret.to_string());
        Ok(())
    }

    pub(super) fn read(account: &str) -> Result<Option<String>, String> {
        Ok(STORE.lock().unwrap().get(account).cloned())
    }

    pub(super) fn delete(account: &str) -> Result<(), String> {
        STORE.lock().unwrap().remove(account);
        Ok(())
    }
}

// ── db-runtime WIT import — schedule async queries ──────────────────────────

impl thoth::plugin::db_runtime::Host for DataSourcePluginState {
    fn submit_query(&mut self, handle: String, q: String) -> String {
        let req_id = next_query_request_id();
        let _ = self.query_request_tx.send((req_id.clone(), handle, q));
        req_id
    }
}

// ── signals WIT import — plugin PUSHes status/metric key-values ───────────────

impl thoth::plugin::signals::Host for DataSourcePluginState {
    fn emit_signal(
        &mut self,
        key: String,
        value: String,
        status: thoth::plugin::signals::Status,
        ttl_ms: u32,
    ) {
        use crate::plugin::signals::SignalStatus;
        let status = match status {
            thoth::plugin::signals::Status::Ready => SignalStatus::Ready,
            thoth::plugin::signals::Status::Loading => SignalStatus::Loading,
            thoth::plugin::signals::Status::Error => SignalStatus::Error,
        };
        crate::plugin::signals::emit(&self.plugin_id, key, value, status, ttl_ms);
    }
}

// ── file-dialog WIT import — native open/save pickers (host-mediated I/O) ─────

fn fd_err(message: impl Into<String>) -> thoth::plugin::file_dialog::PluginError {
    thoth::plugin::file_dialog::PluginError {
        code: 1,
        message: message.into(),
    }
}

/// Apply `title` and an optional single suffix filter to a file dialog.
fn fd_dialog(title: &str, extensions: &[String]) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new();
    if !title.is_empty() {
        dialog = dialog.set_title(title);
    }
    if !extensions.is_empty() {
        let exts: Vec<&str> = extensions.iter().map(String::as_str).collect();
        dialog = dialog.add_filter("", &exts);
    }
    dialog
}

impl thoth::plugin::file_dialog::Host for DataSourcePluginState {
    fn open_file(
        &mut self,
        title: String,
        extensions: Vec<String>,
    ) -> std::result::Result<
        Option<thoth::plugin::file_dialog::OpenedFile>,
        thoth::plugin::file_dialog::PluginError,
    > {
        let Some(path) = fd_dialog(&title, &extensions).pick_file() else {
            return Ok(None);
        };
        let contents = std::fs::read_to_string(&path).map_err(|e| fd_err(e.to_string()))?;
        Ok(Some(thoth::plugin::file_dialog::OpenedFile {
            path: path.to_string_lossy().into_owned(),
            contents,
        }))
    }

    fn save_file(
        &mut self,
        title: String,
        default_name: String,
        extensions: Vec<String>,
        contents: String,
    ) -> std::result::Result<Option<String>, thoth::plugin::file_dialog::PluginError> {
        let mut dialog = fd_dialog(&title, &extensions);
        if !default_name.is_empty() {
            dialog = dialog.set_file_name(&default_name);
        }
        let Some(path) = dialog.save_file() else {
            return Ok(None);
        };
        std::fs::write(&path, contents).map_err(|e| fd_err(e.to_string()))?;
        Ok(Some(path.to_string_lossy().into_owned()))
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
    /// `Arc<Mutex>` so async query workers can own the Store off the UI thread.
    inner: Arc<Mutex<WasmDataSourceInner>>,
    consent_rx: std::sync::mpsc::Receiver<ConsentRequest>,
    /// Receives completed async HTTP results submitted via `submit()`.
    http_rx: std::sync::mpsc::Receiver<(String, HttpCallResult)>,
    /// Receives retry requests from consent-approved callbacks.
    retry_rx: std::sync::mpsc::Receiver<(String, thoth::plugin::http_client::HttpRequest)>,
    /// Number of submitted requests that haven't been drained yet.
    pending_count: Arc<AtomicUsize>,
    /// Receives tab-open requests raised by the plugin via the `ui-tabs` import.
    tab_rx: std::sync::mpsc::Receiver<TabOpenRequest>,
    /// Receives async query requests from `db-runtime::submit-query`.
    query_request_rx: std::sync::mpsc::Receiver<QueryRequest>,
    /// Cloned into each spawned query worker to deliver its result.
    query_result_tx: std::sync::mpsc::Sender<(String, QueryResult)>,
    /// Receives completed async query results.
    query_result_rx: std::sync::mpsc::Receiver<(String, QueryResult)>,
    /// In-flight async queries (for repaint-while-pending).
    query_pending: Arc<AtomicUsize>,
    plugin_id: String,
    /// Last rendered sidebar/main-UI trees. When a query worker owns the Store
    /// (a blocking DB query is running), the render path reuses these instead of
    /// blocking the UI thread on the Store mutex.
    last_sidebar: Mutex<Option<UiOutput>>,
    last_ui: Mutex<Option<UiOutput>>,
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
        let (tab_tx, tab_rx) = std::sync::mpsc::channel::<TabOpenRequest>();
        let (query_request_tx, query_request_rx) = std::sync::mpsc::channel::<QueryRequest>();
        let (query_result_tx, query_result_rx) =
            std::sync::mpsc::channel::<(String, QueryResult)>();
        let pending_count = Arc::new(AtomicUsize::new(0));
        let query_pending = Arc::new(AtomicUsize::new(0));
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
            tab_tx,
            tcp_streams: HashMap::new(),
            next_tcp_id: 1,
            query_request_tx,
            current_query: None,
            query_result_tx: query_result_tx.clone(),
            query_pending: Arc::clone(&query_pending),
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

        // 4. Register the ui-tabs import (plugin-initiated tab opening).
        thoth::plugin::ui_tabs::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s).map_err(
            |e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            },
        )?;

        // 5. Register the tcp-client + secure-storage imports (DB plugins).
        thoth::plugin::tcp_client::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s).map_err(
            |e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            },
        )?;
        thoth::plugin::secure_storage::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s).map_err(
            |e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            },
        )?;

        // 6. Register the db-runtime import (async query scheduling).
        thoth::plugin::db_runtime::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s).map_err(
            |e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            },
        )?;

        // 7. Register the file-dialog import (native open/save pickers).
        thoth::plugin::file_dialog::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s).map_err(
            |e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            },
        )?;

        // 8. Register the signals import (plugin PUSHes status-bar signals).
        thoth::plugin::signals::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s).map_err(
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
            inner: Arc::new(Mutex::new(WasmDataSourceInner { store, bindings })),
            consent_rx,
            http_rx,
            retry_rx,
            pending_count,
            tab_rx,
            query_request_rx,
            query_result_tx,
            query_result_rx,
            query_pending,
            plugin_id,
            last_sidebar: Mutex::new(None),
            last_ui: Mutex::new(None),
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
    pub fn on_setting_change(&self, settings: &[PluginSettingData]) -> Result<()> {
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
    /// Render the plugin's sidebar tree. Uses `try_lock`: if a query worker owns
    /// the Store (a blocking DB query is in flight), reuse the last rendered
    /// frame so the UI thread never blocks. Spinner nodes in that cached tree
    /// keep animating because the host re-renders it each frame.
    pub fn render_sidebar(&self) -> Result<Option<UiOutput>> {
        let mut guard = match self.inner.try_lock() {
            Ok(g) => g,
            Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                return Ok(self
                    .last_sidebar
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone());
            }
        };
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        refuel(store)?;
        let result = bindings
            .thoth_plugin_ui_component()
            .call_render_sidebar(store)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })?;
        let out = result.map(|o| UiOutput {
            node_json: o.node_json,
            height_hint: o.height_hint,
        });
        *self.last_sidebar.lock().unwrap_or_else(|e| e.into_inner()) = out.clone();
        Ok(out)
    }

    /// Ask the plugin to render its initial UI tree. Like [`render_sidebar`], it
    /// reuses the last frame rather than blocking the UI thread while a query
    /// worker owns the Store.
    pub fn render_ui(&self) -> Result<UiOutput> {
        let mut guard = match self.inner.try_lock() {
            Ok(g) => g,
            Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                if let Some(cached) = self
                    .last_ui
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
                {
                    return Ok(cached);
                }
                // No cached frame yet — block once (only possible on the very
                // first render, before any query is in flight).
                self.inner.lock().unwrap_or_else(|e| e.into_inner())
            }
        };
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        refuel(store)?;
        let wit_out = bindings
            .thoth_plugin_ui_component()
            .call_render_ui(store)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })?;
        let out = UiOutput {
            node_json: wit_out.node_json,
            height_hint: wit_out.height_hint,
        };
        *self.last_ui.lock().unwrap_or_else(|e| e.into_inner()) = Some(out.clone());
        Ok(out)
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

    /// Drain queued `submit-query` requests and run each on its own worker thread,
    /// which owns the Store (via the shared `Arc<Mutex>`) for the query's duration —
    /// so a blocking DB query runs off the UI thread. Results are delivered via
    /// `drain_query_results`. Call once per poll.
    pub fn pump_queries(&self) {
        while let Ok((req_id, handle, sql)) = self.query_request_rx.try_recv() {
            let inner = Arc::clone(&self.inner);
            let tx = self.query_result_tx.clone();
            self.query_pending.fetch_add(1, Ordering::Relaxed);
            std::thread::spawn(move || {
                let result: QueryResult = {
                    let mut guard = inner.lock().unwrap_or_else(|e| e.into_inner());
                    let WasmDataSourceInner { store, bindings } = &mut *guard;
                    // Record the in-flight query so a consent-gated tcp connect can
                    // re-enqueue it after the user approves the host.
                    store.data_mut().current_query =
                        Some((req_id.clone(), handle.clone(), sql.clone()));
                    let out = match refuel(store) {
                        Err(e) => Err(e.to_string()),
                        Ok(()) => match bindings.thoth_plugin_data_source().call_query(
                            &mut *store,
                            &handle,
                            &sql,
                        ) {
                            Ok(Ok(json)) => Ok(json),
                            Ok(Err(pe)) => Err(pe.message),
                            Err(e) => Err(e.to_string()),
                        },
                    };
                    store.data_mut().current_query = None;
                    out
                };
                let _ = tx.send((req_id, result));
            });
        }
    }

    /// Non-blocking drain of completed async query results: `(request_id, result)`.
    pub fn drain_query_results(&self) -> Vec<(String, QueryResult)> {
        let mut out = Vec::new();
        while let Ok(item) = self.query_result_rx.try_recv() {
            self.query_pending.fetch_sub(1, Ordering::Relaxed);
            out.push(item);
        }
        out
    }

    /// True while at least one async query is still running.
    pub fn has_pending_query(&self) -> bool {
        self.query_pending.load(Ordering::Relaxed) > 0
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

    // ── tab-host export ─────────────────────────────────────────────────────────

    pub fn tab_title(&self) -> Option<String> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        refuel(store).ok()?;
        bindings.thoth_plugin_tab_host().call_tab_title(store).ok()
    }

    pub fn tab_icon(&self) -> Option<String> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        refuel(store).ok()?;
        bindings
            .thoth_plugin_tab_host()
            .call_tab_icon(store)
            .ok()
            .flatten()
    }

    pub fn get_state(&self) -> Result<Option<String>> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let WasmDataSourceInner { store, bindings } = &mut *guard;
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
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        refuel(store)?;
        bindings
            .thoth_plugin_tab_host()
            .call_init_with_state(store, state)
            .map_err(|e| ThothError::Unknown {
                message: e.to_string(),
            })?
            .map_err(|e| ThothError::Unknown { message: e.message })
    }

    fn call_tab_lifecycle(&self, which: u8) {
        // Best-effort: skip if a query worker currently owns the Store, so e.g.
        // switching tabs during a running query never blocks the UI thread.
        let Ok(mut guard) = self.inner.try_lock() else {
            return;
        };
        let WasmDataSourceInner { store, bindings } = &mut *guard;
        if refuel(store).is_err() {
            return;
        }
        let host = bindings.thoth_plugin_tab_host();
        let _ = match which {
            0 => host.call_on_tab_focused(store),
            1 => host.call_on_tab_blurred(store),
            _ => host.call_on_tab_closed(store),
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

// ── PluginUiHost — lets an ActivePluginPane hold this loader as a trait object ──

fn http_req_to_plugin(r: thoth::plugin::http_client::HttpRequest) -> PluginHttpRequest {
    PluginHttpRequest {
        url: r.url,
        method: r.method,
        headers: r.headers,
        body: r.body,
    }
}

fn plugin_req_to_http(r: PluginHttpRequest) -> thoth::plugin::http_client::HttpRequest {
    thoth::plugin::http_client::HttpRequest {
        url: r.url,
        method: r.method,
        headers: r.headers,
        body: r.body,
    }
}

impl PluginUiHost for WasmDataSourceLoader {
    fn plugin_id(&self) -> &str {
        WasmDataSourceLoader::plugin_id(self)
    }

    fn render_ui(&self) -> Result<UiOutput> {
        WasmDataSourceLoader::render_ui(self)
    }

    fn handle_event(&self, event: UiEvent) -> Result<UiOutput> {
        WasmDataSourceLoader::handle_event(self, event)
    }

    fn render_sidebar(&self) -> Result<Option<UiOutput>> {
        WasmDataSourceLoader::render_sidebar(self)
    }

    fn busy(&self) -> bool {
        // A background query worker holds the Store mutex while a blocking DB
        // query runs; `try_lock` failing means it's busy.
        matches!(
            self.inner.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        )
    }

    fn on_setting_change(&self, settings: &[PluginSettingData]) -> Result<()> {
        WasmDataSourceLoader::on_setting_change(self, settings)
    }

    fn tab_title(&self) -> Option<String> {
        WasmDataSourceLoader::tab_title(self).filter(|s| !s.is_empty())
    }

    fn tab_icon(&self) -> Option<String> {
        WasmDataSourceLoader::tab_icon(self)
    }

    fn get_state(&self) -> Result<Option<String>> {
        WasmDataSourceLoader::get_state(self)
    }

    fn init_with_state(&self, state: &str) -> Result<()> {
        WasmDataSourceLoader::init_with_state(self, state)
    }

    fn on_tab_focused(&self) {
        self.call_tab_lifecycle(0);
    }

    fn on_tab_blurred(&self) {
        self.call_tab_lifecycle(1);
    }

    fn on_tab_closed(&self) {
        self.call_tab_lifecycle(2);
    }

    fn drain_tab_open_requests(&self) -> Vec<TabOpenRequest> {
        WasmDataSourceLoader::drain_tab_open_requests(self)
    }

    fn drain_http_results(&self) -> Vec<(String, HttpCallResult)> {
        WasmDataSourceLoader::drain_http_results(self)
    }

    fn drain_retry_requests(&self) -> Vec<(String, PluginHttpRequest)> {
        WasmDataSourceLoader::drain_retry_requests(self)
            .into_iter()
            .map(|(id, req)| (id, http_req_to_plugin(req)))
            .collect()
    }

    fn dispatch_approved_request(&self, request_id: String, req: PluginHttpRequest) {
        WasmDataSourceLoader::dispatch_approved_request(self, request_id, plugin_req_to_http(req));
    }

    fn has_pending_http(&self) -> bool {
        WasmDataSourceLoader::has_pending_http(self)
    }

    fn pump_queries(&self) {
        WasmDataSourceLoader::pump_queries(self)
    }

    fn drain_query_results(&self) -> Vec<(String, QueryResult)> {
        WasmDataSourceLoader::drain_query_results(self)
    }

    fn has_pending_query(&self) -> bool {
        WasmDataSourceLoader::has_pending_query(self)
    }
}

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn tcp_err_carries_code_and_message() {
        let e = tcp_err(403, "blocked");
        assert_eq!(e.code, 403);
        assert_eq!(e.message, "blocked");
    }

    #[test]
    fn se_err_defaults_to_code_1() {
        let e = se_err("nope");
        assert_eq!(e.code, 1);
        assert_eq!(e.message, "nope");
    }

    #[test]
    fn secret_store_roundtrip_in_memory() {
        // Under cfg(test) the secret store is an in-process map — never the real
        // OS keychain — so write/read/delete round-trips here.
        let key = "helper_tests:roundtrip";
        secret_store::write(key, "s3cret").unwrap();
        assert_eq!(secret_store::read(key).unwrap().as_deref(), Some("s3cret"));
        secret_store::delete(key).unwrap();
        assert_eq!(secret_store::read(key).unwrap(), None);
    }

    #[test]
    fn secret_store_read_absent_is_none() {
        assert_eq!(secret_store::read("helper_tests:absent").unwrap(), None);
    }

    #[test]
    fn box_io_passes_reads_and_writes_through() {
        use std::io::{Cursor, Read, Write};
        let sink: Box<dyn ReadWrite> = Box::new(Cursor::new(Vec::new()));
        let mut w = BoxIo(sink);
        assert_eq!(w.write(b"hello").unwrap(), 5);
        w.flush().unwrap();

        let src: Box<dyn ReadWrite> = Box::new(Cursor::new(b"world".to_vec()));
        let mut r = BoxIo(src);
        let mut buf = [0u8; 5];
        r.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"world");
    }

    #[test]
    fn accept_any_server_cert_accepts_everything() {
        use rustls::client::danger::ServerCertVerifier;
        use rustls::pki_types::{CertificateDer, ServerName, UnixTime};

        let verifier = AcceptAnyServerCert;
        let cert = CertificateDer::from(vec![0u8, 1, 2, 3]); // contents are not inspected
        let name = ServerName::try_from("db.internal").unwrap();
        let now = UnixTime::since_unix_epoch(std::time::Duration::from_secs(1_700_000_000));

        assert!(
            verifier
                .verify_server_cert(&cert, &[], &name, &[], now)
                .is_ok()
        );
        assert!(!verifier.supported_verify_schemes().is_empty());
    }
}

#[cfg(test)]
mod live_db_tests {
    use super::*;
    use crate::plugin::NetworkDeclarations;
    use crate::plugin::render_node::UiEvent;
    use crate::settings::PluginNetworkPolicy;
    use wasmtime::Config;

    /// Snapshots a plugin's on-disk state file and restores it on drop, so tests
    /// that write through plugin-storage don't pollute the real app data dir.
    struct PluginStateGuard {
        path: Option<std::path::PathBuf>,
        original: Option<String>,
    }
    impl PluginStateGuard {
        fn capture(plugin_id: &str) -> Self {
            let path = PersistentState::plugin_state_path(plugin_id).ok();
            let original = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
            Self { path, original }
        }
    }
    impl Drop for PluginStateGuard {
        fn drop(&mut self) {
            if let Some(path) = &self.path {
                match &self.original {
                    Some(s) => {
                        let _ = std::fs::write(path, s);
                    }
                    None => {
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
        }
    }

    /// A `*`-allowlisted policy (matches Seshat's plugin.toml) so the tcp-client
    /// connect to a local DB is permitted without a consent round-trip.
    fn wildcard_policy() -> NetworkPolicy {
        let plugin = NetworkDeclarations {
            allowed_domains: vec!["*".to_string()],
            require_https: false,
            rate_limit_rpm: 120,
        };
        let user = PluginNetworkPolicy {
            allowed_domains: vec!["*".to_string()],
            blocked_domains: vec![],
            require_https: false,
            rate_limit_rpm: 120,
        };
        NetworkPolicy::from_plugin_and_settings(&plugin, &user)
    }

    fn ev(id: &str, value: &str) -> UiEvent {
        UiEvent {
            widget_id: id.to_string(),
            kind: "change".to_string(),
            value: value.to_string(),
        }
    }

    /// End-to-end exercise of the real wasm path against a live Postgres:
    /// instantiate the bundled Seshat plugin, set the connection, and run a
    /// `SELECT *` through the data-source `query` export. This proves the
    /// postgres-protocol codec + SCRAM auth run *inside* wasm (WASI random) and
    /// that the host tcp-client transport connects for real.
    ///
    /// Ignored by default (needs a database). Configure via env and run:
    ///   SESHAT_PG_HOST=127.0.0.1 SESHAT_PG_PORT=5432 \
    ///   SESHAT_PG_DB=... SESHAT_PG_USER=... SESHAT_PG_PASSWORD=... \
    ///   SESHAT_PG_SQL='SELECT * FROM some_table LIMIT 3' \
    ///   cargo test -p thoth --lib seshat_select_star_live_postgres -- --ignored --nocapture
    #[test]
    #[ignore = "requires a live Postgres; configure with SESHAT_PG_* env vars"]
    fn seshat_select_star_live_postgres() {
        let (Ok(host), Ok(db), Ok(user), Ok(password)) = (
            std::env::var("SESHAT_PG_HOST"),
            std::env::var("SESHAT_PG_DB"),
            std::env::var("SESHAT_PG_USER"),
            std::env::var("SESHAT_PG_PASSWORD"),
        ) else {
            eprintln!("skipping: set SESHAT_PG_HOST/DB/USER/PASSWORD to run");
            return;
        };
        let port = std::env::var("SESHAT_PG_PORT").unwrap_or_else(|_| "5432".to_string());
        let sql = std::env::var("SESHAT_PG_SQL").unwrap_or_else(|_| {
            "SELECT * FROM _prisma_migrations ORDER BY started_at LIMIT 3".to_string()
        });

        let wasm = Path::new("assets/plugins/seshat/plugin.wasm");
        assert!(
            wasm.exists(),
            "build first (cargo build) so the plugin is bundled"
        );

        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).expect("engine");

        let loader = WasmDataSourceLoader::open(
            &engine,
            wasm,
            wildcard_policy(),
            "com.thoth.seshat".to_string(),
            &[],
        )
        .expect("open seshat plugin");

        for (id, v) in [
            ("host", host.as_str()),
            ("port", port.as_str()),
            ("database", db.as_str()),
            ("user", user.as_str()),
            ("password", password.as_str()),
        ] {
            loader
                .handle_event(ev(id, v))
                .expect("set connection field");
        }

        // Helper: run a Request (the plugin's off-thread op envelope) and parse.
        let call = |req: serde_json::Value| -> serde_json::Value {
            let json = loader
                .query("seshat", &req.to_string())
                .unwrap_or_else(|e| panic!("query {req} failed: {e:?}"));
            serde_json::from_str(&json).expect("result json")
        };

        // test_connection → server version string
        let version = call(serde_json::json!({"op": "test_connection"}));
        eprintln!("test_connection: {version}");
        assert!(
            version.as_str().unwrap_or_default().contains("PostgreSQL"),
            "expected a PostgreSQL version banner"
        );

        // list_schemas → contains the default `public` schema
        let schemas = call(serde_json::json!({"op": "list_schemas"}));
        eprintln!("schemas: {schemas}");
        assert!(
            schemas
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s.as_str() == Some("public")),
            "expected a `public` schema"
        );

        // list_tables(public) → returns {schema,name,kind} objects
        let tables = call(serde_json::json!({"op": "list_tables", "schema": "public"}));
        let table_count = tables.as_array().map(|a| a.len()).unwrap_or(0);
        eprintln!("public has {table_count} table(s)");
        assert!(table_count > 0, "expected at least one table in public");
        let first_table = tables.as_array().unwrap()[0]["name"]
            .as_str()
            .expect("table name")
            .to_string();

        // list_columns(public, <first table>) → typed column metadata
        let columns = call(serde_json::json!({
            "op": "list_columns", "schema": "public", "table": first_table
        }));
        eprintln!("columns of {first_table}: {columns}");
        assert!(
            !columns.as_array().unwrap().is_empty(),
            "expected columns for {first_table}"
        );
        assert!(
            columns.as_array().unwrap()[0].get("data_type").is_some(),
            "each column should carry a data_type"
        );

        // query → typed {columns:[{name,type}], rows:[[..]]}
        let result = call(serde_json::json!({"op": "query", "sql": sql}));
        eprintln!(
            "query result:\n{}",
            serde_json::to_string_pretty(&result).unwrap()
        );
        let cols = result["columns"].as_array().expect("columns array");
        let rows = result["rows"].as_array().expect("rows array");
        assert!(!cols.is_empty(), "expected typed columns from `{sql}`");
        assert!(!rows.is_empty(), "expected at least one row from `{sql}`");
        assert!(
            cols[0].get("name").is_some() && cols[0].get("type").is_some(),
            "each column should have a name and type"
        );
        assert!(
            rows[0].is_array(),
            "rows should be positional arrays aligned with columns"
        );
    }

    /// Render the connections UI and the opened new-connection modal, and assert
    /// the host can parse both into `UiNode`. A single bad DSL field would make
    /// the pane render blank in the app; this catches that without a GUI or DB.
    #[test]
    fn seshat_connection_ui_parses() {
        let wasm = Path::new("assets/plugins/seshat/plugin.wasm");
        if !wasm.exists() {
            eprintln!("skipping: build the workspace first so the plugin is bundled");
            return;
        }
        // This test drives `dialog-connect`, which persists via plugin-storage to
        // the real app data dir. Back the file up and restore it on drop so the
        // test never leaves a phantom connection behind.
        let _state_guard = PluginStateGuard::capture("com.thoth.seshat");

        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).expect("engine");
        let loader = WasmDataSourceLoader::open(
            &engine,
            wasm,
            wildcard_policy(),
            "com.thoth.seshat".to_string(),
            &[],
        )
        .expect("open seshat plugin");

        let parse = |json: &str, what: &str| -> UiNode {
            serde_json::from_str(json)
                .unwrap_or_else(|e| panic!("{what} did not parse as UiNode: {e}\n{json}"))
        };

        // Initial view: connections manager with the modal closed.
        let initial = loader.render_ui().expect("render_ui");
        parse(&initial.node_json, "connections view");
        assert!(
            initial.node_json.contains("new-connection"),
            "expected a New-connection button in the connections view"
        );

        // Sidebar view must also parse (this is where bad enum casing surfaced).
        let sidebar = loader
            .render_sidebar()
            .expect("render_sidebar")
            .expect("sidebar output");
        parse(&sidebar.node_json, "sidebar view");
        assert!(
            sidebar.node_json.contains("new-connection"),
            "sidebar should expose a New-connection button"
        );

        // Click "New connection" → the modal must open and still parse.
        let opened = loader
            .handle_event(UiEvent {
                widget_id: "new-connection".to_string(),
                kind: "click".to_string(),
                value: String::new(),
            })
            .expect("handle_event(new-connection)");
        parse(&opened.node_json, "new-connection modal");
        assert!(
            opened.node_json.contains("\"type\":\"modal\"")
                && opened.node_json.contains("\"open\":true"),
            "clicking New connection should open the modal:\n{}",
            opened.node_json
        );

        // Cancel → the modal closes again.
        let closed = loader
            .handle_event(UiEvent {
                widget_id: "dialog-cancel".to_string(),
                kind: "click".to_string(),
                value: String::new(),
            })
            .expect("handle_event(dialog-cancel)");
        parse(&closed.node_json, "after cancel");
        assert!(
            closed.node_json.contains("\"open\":false"),
            "Cancel should close the modal"
        );

        // Connecting saves the connection and activates it (no tab opened);
        // render_ui then shows the editor view.
        loader
            .handle_event(UiEvent {
                widget_id: "dialog-connect".to_string(),
                kind: "click".to_string(),
                value: String::new(),
            })
            .expect("handle_event(dialog-connect)");
        let editor = loader.render_ui().expect("render_ui (editor)");
        parse(&editor.node_json, "editor view");
        assert!(
            editor.node_json.contains("code-editor"),
            "editor view should have a SQL editor:\n{}",
            editor.node_json
        );

        // Seeding a fresh tab via init_with_state (the table/history open path)
        // also lands on the editor and parses.
        loader
            .init_with_state(r#"{"connection":"localhost","sql":"SELECT 1"}"#)
            .expect("init_with_state");
        let seeded = loader.render_ui().expect("render_ui (seeded)");
        parse(&seeded.node_json, "seeded editor view");
    }
}
