mod helper;

#[rustfmt::skip]
mod bindings;
mod http;
mod ui;

use serde_json::Value;

use thoth_plugin_sdk::PluginMeta;
use thoth_plugin_sdk::state::PluginState;

use bindings::exports::thoth::plugin::{
    data_source::{
        ConfigEntry, FieldSchema, Guest as DataSourceGuest, PaneOutput, PluginError, SourceSchema,
    },
    plugin_lifecycle::Guest as LifecycleGuest,
    plugin_settings::{Guest as SettingsGuest, SettingsOutput},
    tab_host::Guest as TabHostGuest,
    ui_component::{Guest as UiComponentGuest, UiEvent, UiOutput},
};

use crate::{
    helper::{ce, normalise_array, plugin_err, request_is_non_empty, type_hint, ui_out},
    http::http_fetch,
};

#[derive(PluginMeta)]
#[plugin(
    id = "com.thoth.url-source",
    name = "URL Source",
    version = "0.1.0",
    description = "Fetch JSON data from any HTTP endpoint",
    capabilities = [DataSource, NewUiComponent],
    author = "Thoth contributors",
    icon = "\u{E28C}",
)]
struct UrlSourcePlugin;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct State {
    // ── Saved requests ────────────────────────────────────────────────────
    request_name: String, // name input in the sidebar
    #[serde(default)]
    saved_requests: Vec<SavedRequest>,

    // ── Request ───────────────────────────────────────────────────────────
    url: String,
    method: String,
    params: Vec<KvPair>,      // query params
    req_headers: Vec<KvPair>, // custom request headers
    body: String,

    // ── Auth ──────────────────────────────────────────────────────────────
    auth_type: String, // "none" | "bearer" | "basic" | "api-key"
    auth_token: String,
    auth_username: String,
    auth_password: String,
    auth_key_name: String,
    auth_key_value: String,
    auth_key_in: String, // "header" | "query"

    // ── UI navigation ─────────────────────────────────────────────────────
    resp_tab: String, // "pretty" | "raw" | "headers"

    // ── cURL export / import modals ──────────────────────────────────────
    #[serde(skip)]
    show_export_modal: bool,
    #[serde(skip)]
    show_import_modal: bool,
    #[serde(skip)]
    curl_import_input: String,

    // ── Async fetch state (transient — never persisted) ───────────────────
    /// True while a submit() request is in flight.
    #[serde(skip)]
    loading: bool,
    /// True when loading is paused waiting for the user to approve a consent popup.
    #[serde(skip)]
    consent_pending: bool,
    /// The request_id returned by submit(); matched against the http-response event.
    #[serde(skip)]
    pending_request_id: Option<String>,

    // ── Response cache (transient — never persisted) ──────────────────────
    #[serde(skip)]
    response: Option<ResponseState>,
}

impl State {
    fn fresh() -> Self {
        Self {
            request_name: String::new(),
            saved_requests: Vec::new(),
            url: String::new(),
            method: "GET".to_string(),
            params: Vec::new(),
            req_headers: Vec::new(),
            body: String::new(),
            auth_type: "none".to_string(),
            auth_token: String::new(),
            auth_username: String::new(),
            auth_password: String::new(),
            auth_key_name: String::new(),
            auth_key_value: String::new(),
            auth_key_in: "header".to_string(),
            resp_tab: "pretty".to_string(),
            show_export_modal: false,
            show_import_modal: false,
            curl_import_input: String::new(),
            loading: false,
            consent_pending: false,
            pending_request_id: None,
            response: None,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        State::fresh()
    }
}

#[derive(Clone, Default, Debug)]
struct ResponseState {
    status: u16,
    headers: Vec<KvPair>,
    body: String, // pretty-printed if JSON, raw otherwise
    /// Pre-parsed JSON value, populated when the body is valid JSON so
    /// build_response_panel doesn't need to re-parse on every render frame.
    parsed_body: Option<serde_json::Value>,
    error: Option<String>,
    /// Round-trip time in milliseconds, if provided by the host.
    duration_ms: Option<u64>,
    /// Response body size in bytes.
    size_bytes: usize,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
struct KvPair {
    key: String,
    value: String,
    /// Whether this row is sent. Disabled rows are kept (so the user can re-enable
    /// them) but skipped when building the request / cURL. Defaults to true.
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for KvPair {
    fn default() -> Self {
        Self {
            key: String::new(),
            value: String::new(),
            enabled: true,
        }
    }
}

/// A single saved request entry.
#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
struct SavedRequest {
    name: String,
    method: String,
    url: String,
    params: Vec<KvPair>,
    req_headers: Vec<KvPair>,
    body: String,
    auth_type: String,
    auth_token: String,
    auth_username: String,
    auth_password: String,
    auth_key_name: String,
    auth_key_value: String,
    auth_key_in: String,
}

/// On-disk format — wraps the list of saved requests.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct StoredData {
    requests: Vec<SavedRequest>,
}

static STATE: PluginState<State> = PluginState::new();

impl LifecycleGuest for UrlSourcePlugin {
    fn on_load(setting: String) {
        // The host passes PluginSettingData (key/value config) in `setting`.
        // url-source has no host-configurable settings, so we ignore those
        // entries and restore plugin state from plugin_storage instead.
        let _ = setting;
        let raw = bindings::thoth::plugin::plugin_storage::read();
        if raw.is_empty() {
            return;
        }
        if let Ok(data) = serde_json::from_str::<StoredData>(&raw) {
            STATE.with_mut(|s| {
                s.saved_requests = data.requests;
            });
        }
    }
    fn on_close() {
        STATE.reset();
    }
    fn on_setting_change(_setting: String) {
        // Persist current saved requests so host-driven reloads see fresh state.
        STATE.with(|s| {
            persist_requests(&s.saved_requests);
        });
    }
}

impl DataSourceGuest for UrlSourcePlugin {
    fn required_config() -> Vec<ConfigEntry> {
        vec![
            ce("url", "Request URL", true, ""),
            ce("method", "HTTP method", false, "GET"),
            ce("headers", "Extra headers as JSON object", false, "{}"),
            ce("body", "Request body (POST/PUT/PATCH only)", false, ""),
            ce(
                "auth_type",
                "none | bearer | basic | api-key",
                false,
                "none",
            ),
            ce("auth_token", "Bearer token", false, ""),
            ce("auth_username", "Basic auth username", false, ""),
            ce("auth_password", "Basic auth password", false, ""),
            ce(
                "auth_key_name",
                "Header name or query param for API key",
                false,
                "",
            ),
            ce("auth_key_value", "API key value", false, ""),
            ce("auth_key_in", "header | query", false, "header"),
        ]
    }

    fn connect(config: Vec<ConfigEntry>) -> Result<String, PluginError> {
        STATE.with_mut(|st| {
            for entry in &config {
                match entry.name.as_str() {
                    "url" => st.url = entry.value.clone(),
                    "method" => st.method = entry.value.clone(),
                    "headers" => {
                        if let Ok(obj) = serde_json::from_str::<Value>(&entry.value) {
                            if let Some(map) = obj.as_object() {
                                st.req_headers = map
                                    .iter()
                                    .map(|(k, v)| KvPair {
                                        key: k.clone(),
                                        value: v.as_str().unwrap_or("").to_string(),
                                        enabled: true,
                                    })
                                    .collect();
                            }
                        }
                    }
                    "body" => st.body = entry.value.clone(),
                    "auth_type" => st.auth_type = entry.value.clone(),
                    "auth_token" => st.auth_token = entry.value.clone(),
                    "auth_username" => st.auth_username = entry.value.clone(),
                    "auth_password" => st.auth_password = entry.value.clone(),
                    "auth_key_name" => st.auth_key_name = entry.value.clone(),
                    "auth_key_value" => st.auth_key_value = entry.value.clone(),
                    "auth_key_in" => st.auth_key_in = entry.value.clone(),
                    _ => {}
                }
            }

            if st.url.is_empty() {
                return Err(plugin_err(1, "url is required"));
            }
            Ok("connected".to_string())
        })
    }

    fn schema(_handle: String) -> Result<Vec<SourceSchema>, PluginError> {
        STATE.with(|st| {
            // Use cached response if available, otherwise make a request
            let body_str = if let Some(resp) = &st.response {
                if resp.error.is_none() {
                    resp.body.clone()
                } else {
                    return Err(plugin_err(1, resp.error.clone().unwrap()));
                }
            } else {
                let raw = http_fetch(&st)?;
                String::from_utf8_lossy(&raw).to_string()
            };

            let value: Value = serde_json::from_str(&body_str)
                .map_err(|e| plugin_err(3, format!("Invalid JSON: {e}")))?;

            let first = match &value {
                Value::Array(arr) => arr.first().cloned().unwrap_or_default(),
                v => v.clone(),
            };

            let fields = first
                .as_object()
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| FieldSchema {
                            name: k.clone(),
                            type_hint: type_hint(v),
                            nullable: v.is_null(),
                        })
                        .collect()
                })
                .unwrap_or_default();

            Ok(vec![SourceSchema {
                name: "response".to_string(),
                fields,
            }])
        })
    }

    fn query(_handle: String, _q: String) -> Result<String, PluginError> {
        STATE.with(|st| {
            // Return cached result from last UI "Send" if available
            if let Some(resp) = &st.response {
                if resp.error.is_none() {
                    let val: Value = serde_json::from_str(&resp.body)
                        .map_err(|e| plugin_err(3, format!("Invalid cached JSON: {e}")))?;
                    let arr = normalise_array(val);
                    return serde_json::to_string(&arr).map_err(|e| plugin_err(3, e.to_string()));
                }
            }

            // No cache — fresh request
            let raw = http_fetch(&st)?;
            let body_str = String::from_utf8_lossy(&raw).to_string();
            let val: Value = serde_json::from_str(&body_str)
                .map_err(|e| plugin_err(3, format!("Invalid JSON: {e}")))?;
            let arr = normalise_array(val);
            serde_json::to_string(&arr).map_err(|e| plugin_err(3, e.to_string()))
        })
    }

    fn close(_handle: String) {
        STATE.reset();
    }

    fn render_pane(_handle: String) -> Result<PaneOutput, PluginError> {
        STATE.with(|st| {
            Ok(PaneOutput {
                node_json: serde_json::to_string(&build_pane_node(st)).unwrap_or_default(),
                height_hint: 0,
            })
        })
    }
}

/// Build a RenderNode tree for the main pane. (Currently unused — the UI lives
/// in the ui-component surface; render_pane returns an empty column.)
fn build_pane_node(_st: &State) -> thoth_plugin_sdk::render_node::RenderNode {
    thoth_plugin_sdk::render_node::RenderNode::Column(
        thoth_plugin_sdk::components::Column::builder().build(),
    )
}

/// Persist the current list of saved requests to plugin storage.
fn persist_requests(saved_requests: &[SavedRequest]) {
    let data = StoredData {
        requests: saved_requests.to_vec(),
    };
    if let Ok(json) = serde_json::to_string(&data) {
        let _ = bindings::thoth::plugin::plugin_storage::write(&json);
    }
}

/// Save the current request under `st.request_name` (falls back to URL).
fn save_current_request(st: &mut State) {
    if st.url.is_empty() {
        return;
    }
    let name = {
        let trimmed = st.request_name.trim().to_string();
        if trimmed.is_empty() {
            st.url.clone()
        } else {
            trimmed
        }
    };
    let req = SavedRequest {
        name: name.clone(),
        method: st.method.clone(),
        url: st.url.clone(),
        params: st.params.clone(),
        req_headers: st.req_headers.clone(),
        body: st.body.clone(),
        auth_type: st.auth_type.clone(),
        auth_token: st.auth_token.clone(),
        auth_username: st.auth_username.clone(),
        auth_password: st.auth_password.clone(),
        auth_key_name: st.auth_key_name.clone(),
        auth_key_value: st.auth_key_value.clone(),
        auth_key_in: st.auth_key_in.clone(),
    };
    if let Some(existing) = st.saved_requests.iter_mut().find(|r| r.name == name) {
        *existing = req;
    } else {
        st.saved_requests.push(req);
    }
    persist_requests(&st.saved_requests);
}

/// Load a saved request into the active form.
fn load_saved_request(st: &mut State, req: SavedRequest) {
    st.request_name = req.name;
    st.method = req.method;
    st.url = req.url;
    st.params = req.params;
    st.req_headers = req.req_headers;
    st.body = req.body;
    st.auth_type = req.auth_type;
    st.auth_token = req.auth_token;
    st.auth_username = req.auth_username;
    st.auth_password = req.auth_password;
    st.auth_key_name = req.auth_key_name;
    st.auth_key_value = req.auth_key_value;
    st.auth_key_in = req.auth_key_in;
    st.response = None;
    st.loading = false;
    st.consent_pending = false;
    st.pending_request_id = None;
}

/// Build a cURL command string from the current state.
fn build_curl_command(st: &State) -> String {
    use crate::helper::{is_body_method, pct_encode};

    // Build URL with query params
    let url = {
        let active: Vec<&KvPair> = st
            .params
            .iter()
            .filter(|p| p.enabled && !p.key.is_empty())
            .collect();
        if active.is_empty() {
            st.url.clone()
        } else {
            let qs: Vec<String> = active
                .iter()
                .map(|p| format!("{}={}", pct_encode(&p.key), pct_encode(&p.value)))
                .collect();
            let sep = if st.url.contains('?') { '&' } else { '?' };
            format!("{}{}{}", st.url, sep, qs.join("&"))
        }
    };

    let mut parts = vec![format!("curl -X {}", st.method)];

    // Auth header
    match st.auth_type.as_str() {
        "bearer" if !st.auth_token.is_empty() => {
            parts.push(format!("-H 'Authorization: Bearer {}'", st.auth_token));
        }
        "basic" if !st.auth_username.is_empty() => {
            use base64::Engine as _;
            let creds = base64::engine::general_purpose::STANDARD
                .encode(format!("{}:{}", st.auth_username, st.auth_password));
            parts.push(format!("-H 'Authorization: Basic {}'", creds));
        }
        "api-key" if !st.auth_key_name.is_empty() && st.auth_key_in == "header" => {
            parts.push(format!("-H '{}: {}'", st.auth_key_name, st.auth_key_value));
        }
        _ => {}
    }

    // Custom headers
    for h in &st.req_headers {
        if h.enabled && !h.key.is_empty() {
            parts.push(format!("-H '{}: {}'", h.key, h.value));
        }
    }

    // Body
    if is_body_method(&st.method) && !st.body.is_empty() {
        let escaped = st.body.replace('\'', r#"'"'"'"#);
        parts.push("-H 'Content-Type: application/json'".to_string());
        parts.push(format!("-d '{}'", escaped));
    }

    parts.push(format!("'{}'", url));
    parts.join(" \\\n  ")
}

/// Tokenise a shell-like cURL command string.
///
/// Handles:
/// - `\` + newline line-continuation (strips both characters)
/// - Single-quoted strings (`'...'`) — no escape sequences inside
/// - Double-quoted strings (`"..."`) — recognises `\"` and `\\`
/// - Unquoted tokens separated by whitespace
///
/// Returns owned `String` tokens with surrounding quotes already stripped.
fn tokenize_curl(curl: &str) -> Vec<String> {
    // 1. Remove `\` + newline continuations (both CRLF and LF), then strip
    //    any remaining bare CR characters left by Windows line endings.
    let curl = curl
        .replace("\\\r\n", " ")
        .replace("\\\n", " ")
        .replace('\r', "");
    let chars: Vec<char> = curl.chars().collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        // Skip whitespace between tokens.
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }

        let mut token = String::new();
        // Parse until unquoted whitespace.
        while i < chars.len() && !chars[i].is_whitespace() {
            match chars[i] {
                '\'' => {
                    // Single-quoted: collect until closing `'`, no escapes.
                    i += 1;
                    while i < chars.len() && chars[i] != '\'' {
                        token.push(chars[i]);
                        i += 1;
                    }
                    if i < chars.len() {
                        i += 1; // consume closing `'`
                    }
                }
                '"' => {
                    // Double-quoted: recognise `\\` and `\"`.
                    i += 1;
                    while i < chars.len() && chars[i] != '"' {
                        if chars[i] == '\\' && i + 1 < chars.len() {
                            match chars[i + 1] {
                                '"' | '\\' => {
                                    token.push(chars[i + 1]);
                                    i += 2;
                                }
                                _ => {
                                    token.push(chars[i]);
                                    i += 1;
                                }
                            }
                        } else {
                            token.push(chars[i]);
                            i += 1;
                        }
                    }
                    if i < chars.len() {
                        i += 1; // consume closing `"`
                    }
                }
                c => {
                    token.push(c);
                    i += 1;
                }
            }
        }
        if !token.is_empty() {
            tokens.push(token);
        }
    }
    tokens
}

/// Parse a cURL command string into the state fields we care about.
fn apply_curl_import(st: &mut State, curl: &str) {
    use crate::helper::percent_decode;

    let tokens = tokenize_curl(curl);
    // Reset fields that we are about to re-populate.
    st.req_headers.clear();
    st.params.clear();
    st.body.clear();
    st.auth_type = "none".to_string();
    st.auth_token.clear();

    let mut i = 0;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "curl" => {}
            "-X" | "--request" => {
                if let Some(m) = tokens.get(i + 1) {
                    st.method = m.to_uppercase();
                    i += 2;
                    continue;
                }
            }
            "-H" | "--header" => {
                if let Some(h) = tokens.get(i + 1) {
                    if let Some((k, v)) = h.split_once(':') {
                        let k = k.trim().to_string();
                        let v = v.trim().to_string();
                        if k.eq_ignore_ascii_case("authorization") {
                            if let Some(token) = v.strip_prefix("Bearer ") {
                                st.auth_type = "bearer".to_string();
                                st.auth_token = token.to_string();
                            } else if let Some(encoded) = v.strip_prefix("Basic ") {
                                use base64::Engine as _;
                                if let Ok(decoded) =
                                    base64::engine::general_purpose::STANDARD.decode(encoded)
                                {
                                    if let Ok(creds) = String::from_utf8(decoded) {
                                        if let Some((user, pass)) = creds.split_once(':') {
                                            st.auth_type = "basic".to_string();
                                            st.auth_username = user.to_string();
                                            st.auth_password = pass.to_string();
                                        }
                                    }
                                }
                            }
                        } else if !k.eq_ignore_ascii_case("content-type") {
                            st.req_headers.push(crate::KvPair {
                                key: k,
                                value: v,
                                enabled: true,
                            });
                        }
                    }
                    i += 2;
                    continue;
                }
            }
            "-b" | "--cookie" => {
                if let Some(cookie) = tokens.get(i + 1) {
                    st.req_headers.push(crate::KvPair {
                        key: "Cookie".to_string(),
                        value: cookie.clone(),
                        enabled: true,
                    });
                    i += 2;
                    continue;
                }
            }
            t if t.starts_with("--cookie=") => {
                st.req_headers.push(crate::KvPair {
                    key: "Cookie".to_string(),
                    value: t["--cookie=".len()..].to_string(),
                    enabled: true,
                });
            }
            t if t.starts_with("-b") && t.len() > 2 => {
                st.req_headers.push(crate::KvPair {
                    key: "Cookie".to_string(),
                    value: t[2..].to_string(),
                    enabled: true,
                });
            }
            "-d" | "--data" | "--data-raw" => {
                if let Some(body) = tokens.get(i + 1) {
                    st.body = body.clone();
                    if st.method == "GET" {
                        st.method = "POST".to_string();
                    }
                    i += 2;
                    continue;
                }
            }
            t if !t.starts_with('-') && (t.starts_with("http://") || t.starts_with("https://")) => {
                if let Some((base, query)) = t.split_once('?') {
                    st.url = base.to_string();
                    st.params = query
                        .split('&')
                        .filter_map(|kv| {
                            let (k, v) = kv.split_once('=')?;
                            Some(crate::KvPair {
                                key: percent_decode(k),
                                value: percent_decode(v),
                                enabled: true,
                            })
                        })
                        .collect();
                } else {
                    st.url = t.to_string();
                }
            }
            _ => {}
        }
        i += 1;
    }
}

impl UiComponentGuest for UrlSourcePlugin {
    fn render_sidebar() -> Result<Option<UiOutput>, PluginError> {
        STATE.with(|st| Ok(Some(ui_out(ui::build_sidebar(st)))))
    }

    fn render_ui() -> Result<UiOutput, PluginError> {
        STATE.with(|st| Ok(ui_out(ui::build_ui(st))))
    }

    fn handle_event(event: UiEvent) -> Result<UiOutput, PluginError> {
        STATE.with_mut(|st| {
            if event.widget_id == "clear" {
                let saved = st.saved_requests.clone();
                *st = State::fresh();
                st.saved_requests = saved;
            } else if event.widget_id == "save" {
                save_current_request(st);
            } else if event.widget_id == "saved-requests" {
                match event.kind.as_str() {
                    "click" => {
                        // Open the saved request in a NEW TAB (seeded with that
                        // request), rather than loading it into the sidebar form.
                        if let Ok(idx) = event.value.parse::<usize>() {
                            if let Some(req) = st.saved_requests.get(idx).cloned() {
                                let mut seed = st.clone();
                                load_saved_request(&mut seed, req);
                                let title = tab_title_for(&seed);
                                let initial_state = serde_json::to_string(&seed).ok();
                                bindings::thoth::plugin::ui_tabs::open_tab(
                                    &title,
                                    Some("\u{E28C}"),
                                    initial_state.as_deref(),
                                );
                            }
                        }
                    }
                    "action" => {
                        // value = {"item": i, "action": 0}  (action 0 = delete)
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&event.value) {
                            let item_idx =
                                v.get("item").and_then(|x| x.as_u64()).map(|x| x as usize);
                            let action_idx =
                                v.get("action").and_then(|x| x.as_u64()).map(|x| x as usize);
                            if let (Some(item_idx), Some(0)) = (item_idx, action_idx) {
                                if item_idx < st.saved_requests.len() {
                                    st.saved_requests.remove(item_idx);
                                    persist_requests(&st.saved_requests);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            } else if event.widget_id == "open-new-tab" {
                // Open a fresh url-source tab seeded with this request's form.
                let initial_state = serde_json::to_string(&st).ok();
                let title = tab_title_for(st);
                bindings::thoth::plugin::ui_tabs::open_tab(
                    &title,
                    Some("\u{E28C}"),
                    initial_state.as_deref(),
                );
            } else if event.widget_id == "consent-approved" {
                // Host has dispatched the retry request — switch spinner text
                // from "Waiting for consent approval" back to "Sending request".
                st.consent_pending = false;
            } else if event.widget_id == "export-curl" {
                st.show_export_modal = true;
            } else if event.widget_id == "import-curl" {
                st.show_import_modal = true;
                st.curl_import_input = String::new();
            } else if event.widget_id == "close-export" {
                st.show_export_modal = false;
            } else if event.widget_id == "close-import" {
                st.show_import_modal = false;
                st.curl_import_input = String::new();
            } else if event.widget_id == "curl-import-input" {
                st.curl_import_input = crate::helper::parse_str(&event.value);
            } else if event.widget_id == "curl-import-submit" {
                let curl = st.curl_import_input.clone();
                st.show_import_modal = false;
                st.curl_import_input = String::new();
                if request_is_non_empty(st) {
                    // Don't clobber the current request — open the imported one
                    // in a fresh tab seeded with the parsed cURL.
                    let mut seed = st.clone();
                    seed.request_name = String::new();
                    apply_curl_import(&mut seed, &curl);
                    let title = tab_title_for(&seed);
                    let initial_state = serde_json::to_string(&seed).ok();
                    bindings::thoth::plugin::ui_tabs::open_tab(
                        &title,
                        Some("\u{E28C}"),
                        initial_state.as_deref(),
                    );
                } else {
                    apply_curl_import(st, &curl);
                }
            } else {
                ui::apply_event(st, &event);
            }

            Ok(ui_out(ui::build_ui(st)))
        })
    }
}

/// Title shown on the dock tab: the saved request name when set, else `method url`.
fn tab_title_for(st: &State) -> String {
    let name = st.request_name.trim();
    if !name.is_empty() {
        name.to_string()
    } else if st.url.is_empty() {
        "URL Source".to_string()
    } else {
        format!("{} {}", st.method, st.url)
    }
}

impl TabHostGuest for UrlSourcePlugin {
    /// Show the saved request name (or method+url) in the tab label so tabs are
    /// distinguishable.
    fn tab_title() -> String {
        STATE.with(|s| tab_title_for(s))
    }

    fn tab_icon() -> Option<String> {
        Some("\u{E28C}".to_string()) // GLOBE_HEMISPHERE_WEST
    }

    /// Snapshot the per-tab request form so the host can persist it. Transient
    /// fields (response, loading, modals) are `#[serde(skip)]` and not included.
    fn get_state() -> Result<String, PluginError> {
        STATE.with(|s| serde_json::to_string(s).map_err(|e| plugin_err(3, e.to_string())))
    }

    /// Restore the request form from a previously saved snapshot.
    fn init_with_state(state: String) -> Result<(), PluginError> {
        if state.is_empty() {
            return Ok(());
        }
        match serde_json::from_str::<State>(&state) {
            Ok(restored) => STATE.set(restored),
            Err(e) => eprintln!("[url-source] init_with_state: invalid state blob: {e}"),
        }
        Ok(())
    }

    fn on_tab_focused() {
        eprintln!("[url-source] on_tab_focused");
    }

    fn on_tab_blurred() {
        eprintln!("[url-source] on_tab_blurred");
    }

    fn on_tab_closed() {
        eprintln!("[url-source] on_tab_closed");
    }
}

impl SettingsGuest for UrlSourcePlugin {
    fn render_settings() -> Result<SettingsOutput, PluginError> {
        let node = thoth_plugin_sdk::render_node::RenderNode::Text(
            thoth_plugin_sdk::components::Typography::builder()
                .text("URL Source is configured per-request in its panel.")
                .build(),
        );
        Ok(SettingsOutput {
            node_json: serde_json::to_string(&node).unwrap_or_default(),
            height_hint: 0,
        })
    }
}

bindings::export!(UrlSourcePlugin with_types_in bindings);
