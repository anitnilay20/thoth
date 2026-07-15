use crate::{
    bindings::{
        exports::thoth::plugin::ui_component::UiEvent,
        thoth::plugin::{
            http_client,
            signals::{self, Status as SignalStatus},
            websocket,
        },
    },
    helper::{
        is_body_method, parse_kv_list, parse_str, parse_url_into_state, status_color, status_text,
    },
    http::build_request,
    KvPair, ResponseState, State, WsDir, WsLogEntry,
};
use serde_json::Value;
use thoth_plugin_sdk::components::{
    Align, Badge, BgColor, Button, ButtonColor, Code, CodeEditor, Column, DataRow, DataRowIcon,
    IconButton, Input, JsonTree, KeyValueList, KvEntry, List, ListItem, ListItemAction,
    ListItemBadge, Modal, Radio, Row, Scroll, Select, SelectOption, Separator, Spacer, Spinner,
    Split, TabAction, TableView, Tabs, Typography, TypographyVariant,
};
use thoth_plugin_sdk::render_node::RenderNode;

// egui_phosphor::regular glyphs
const ICON_PLUS: &str = "\u{E3D4}"; // PLUS
const ICON_CODE: &str = "\u{E1BC}"; // CODE (export cURL)
const ICON_DOWNLOAD: &str = "\u{E20C}"; // DOWNLOAD_SIMPLE (import cURL)

// ── Small helpers ──────────────────────────────────────────────────────────

/// `#rrggbb` badge colour for an HTTP method.
fn method_badge_hex(method: &str) -> &'static str {
    match method {
        "GET" => "#89b4fa",
        "POST" => "#a6e3a1",
        "PUT" | "PATCH" => "#fab387",
        "DELETE" => "#f38ba8",
        "HEAD" | "OPTIONS" => "#cba6f7",
        _ => "#9399b2",
    }
}

/// Convert the plugin's `KvPair`s into SDK `KvEntry`s.
fn to_entries(pairs: &[KvPair]) -> Vec<KvEntry> {
    pairs
        .iter()
        .map(|p| {
            KvEntry::builder()
                .key(p.key.clone())
                .value(p.value.clone())
                .enabled(p.enabled)
                .build()
        })
        .collect()
}

/// Plain text node.
fn text(value: &str) -> RenderNode {
    RenderNode::Text(Typography::builder().text(value).build())
}

/// Muted text node.
fn muted(value: &str) -> RenderNode {
    RenderNode::Text(
        Typography::builder()
            .text(value)
            .variant(TypographyVariant::BodyMuted)
            .build(),
    )
}

/// An elevated button node.
fn btn(id: &str, label: &str, enabled: bool, color: ButtonColor) -> RenderNode {
    RenderNode::Button(
        Button::builder()
            .id(id)
            .label(label)
            .color(color)
            .enabled(enabled)
            .build(),
    )
}

// ── Main request/response view ──────────────────────────────────────────────

pub fn build_ui(st: &State) -> RenderNode {
    RenderNode::Column(
        Column::builder()
            .gap(0.0)
            .children(vec![
                build_url_bar(st),
                RenderNode::Separator(Separator::plain()),
                RenderNode::Split(
                    Split::builder()
                        .gap(0.0)
                        .separator(true)
                        .fill_height(true)
                        .children(vec![build_request_column(st), build_response_column(st)])
                        .build(),
                ),
                curl_export_modal(st),
            ])
            .build(),
    )
}

/// Rendered by the host in its sidebar panel.
pub fn build_sidebar(st: &State) -> RenderNode {
    let items: Vec<ListItem> = st
        .saved_requests
        .iter()
        .map(|req| {
            ListItem::builder()
                .title(req.name.clone())
                .description(req.url.clone())
                .badge(
                    ListItemBadge::builder()
                        .text(req.method.clone())
                        .color(method_badge_hex(&req.method))
                        .build(),
                )
                .actions(vec![ListItemAction::builder()
                    .icon("x")
                    .tooltip("Delete")
                    .build()])
                .build()
        })
        .collect();

    RenderNode::Column(
        Column::builder()
            .gap(0.0)
            .children(vec![
                RenderNode::Row(
                    Row::builder()
                        .padding(6.0)
                        .children(vec![RenderNode::Text(
                            Typography::builder()
                                .text("COLLECTIONS")
                                .variant(TypographyVariant::PanelHeader)
                                .build(),
                        )])
                        .build(),
                ),
                RenderNode::Row(
                    Row::builder()
                        .gap(6.0)
                        .padding(6.0)
                        .max_width(true)
                        .align(Align::Fill)
                        .children(vec![
                            RenderNode::Button(
                                Button::builder()
                                    .id("open-new-tab")
                                    .label("New Request")
                                    .color(ButtonColor::Primary)
                                    .icon(ICON_PLUS)
                                    .full_width(true)
                                    .build(),
                            ),
                            RenderNode::IconButton(
                                IconButton::builder()
                                    .id("import-curl")
                                    .icon(ICON_DOWNLOAD)
                                    .tooltip("Import cURL")
                                    .frame(true)
                                    .build(),
                            ),
                        ])
                        .build(),
                ),
                RenderNode::Separator(Separator::plain()),
                RenderNode::Row(
                    Row::builder()
                        .gap(4.0)
                        .padding(6.0)
                        .max_width(true)
                        .align(Align::Fill)
                        .children(vec![
                            RenderNode::Input(
                                Input::builder()
                                    .id("request-name")
                                    .value(st.request_name.clone())
                                    .placeholder("Request name")
                                    .grow(true)
                                    .build(),
                            ),
                            btn("save", "Save", !st.url.is_empty(), ButtonColor::Default),
                        ])
                        .build(),
                ),
                RenderNode::Separator(Separator::plain()),
                RenderNode::List(
                    List::builder()
                        .id("saved-requests")
                        .items(items)
                        .empty_label("No saved requests")
                        .build(),
                ),
                curl_import_modal(st),
            ])
            .build(),
    )
}

fn curl_import_modal(st: &State) -> RenderNode {
    RenderNode::Modal(Box::new(
        Modal::builder()
            .id("import-modal")
            .title("Import cURL")
            .open(st.show_import_modal)
            .close_id("close-import")
            .width_pct(0.7)
            .height_pct(0.7)
            .children(vec![
                RenderNode::Input(
                    Input::builder()
                        .id("curl-import-input")
                        .value(st.curl_import_input.clone())
                        .label("cURL command")
                        .placeholder("Paste cURL command here…")
                        .multiline(true)
                        .rows(10)
                        .build(),
                ),
                btn(
                    "curl-import-submit",
                    "Import",
                    !st.curl_import_input.is_empty(),
                    ButtonColor::Default,
                ),
            ])
            .build(),
    ))
}

fn curl_export_modal(st: &State) -> RenderNode {
    let curl = crate::build_curl_command(st);
    RenderNode::Modal(Box::new(
        Modal::builder()
            .id("export-modal")
            .title("Export cURL")
            .open(st.show_export_modal)
            .close_id("close-export")
            .width_pct(0.7)
            .height_pct(0.7)
            .children(vec![
                RenderNode::Input(
                    Input::builder()
                        .id("curl-output")
                        .value(curl.clone())
                        .label("cURL command")
                        .multiline(true)
                        .rows(10)
                        .build(),
                ),
                RenderNode::Button(
                    Button::builder()
                        .label("Copy to Clipboard")
                        .copy(curl)
                        .build(),
                ),
            ])
            .build(),
    ))
}

fn build_request_column(st: &State) -> RenderNode {
    RenderNode::Scroll(
        Scroll::builder()
            .id("request-scroll")
            .child(RenderNode::Column(
                Column::builder()
                    .gap(6.0)
                    .children(vec![build_req_tabs(st)])
                    .build(),
            ))
            .build(),
    )
}

fn build_response_column(st: &State) -> RenderNode {
    if is_ws_mode(st) {
        return build_ws_panel(st);
    }

    if st.loading {
        let label = if st.consent_pending {
            "Waiting for consent approval…"
        } else {
            "Sending request…"
        };
        return RenderNode::Row(
            Row::builder()
                .bg_color(BgColor::BgPanel)
                .max_width(true)
                .padding(10.0)
                .gap(8.0)
                .children(vec![
                    RenderNode::Spinner(Spinner::builder().size(14.0).build()),
                    muted(label),
                ])
                .build(),
        );
    }

    if let Some(resp) = &st.response {
        RenderNode::Scroll(
            Scroll::builder()
                .id("response-scroll")
                .child(build_response_panel(resp))
                .build(),
        )
    } else {
        RenderNode::Row(
            Row::builder()
                .bg_color(BgColor::BgPanel)
                .max_width(true)
                .height(20.0)
                .padding(10.0)
                .children(vec![text("Send a request to see the response here.")])
                .build(),
        )
    }
}

/// WebSocket panel: status header, send box, then the message log (fills the
/// rest and scrolls). Shown in the response column for ws(s):// URLs.
fn build_ws_panel(st: &State) -> RenderNode {
    let status = if st.ws_connected {
        "● connected"
    } else if st.ws_conn_id.is_some() {
        "○ connecting…"
    } else {
        "○ disconnected"
    };

    let log_rows: Vec<RenderNode> = if st.ws_log.is_empty() {
        vec![muted("No messages yet. Connect, then send a frame.")]
    } else {
        st.ws_log
            .iter()
            .enumerate()
            .map(|(i, e)| ws_log_row(i, e))
            .collect()
    };

    RenderNode::Column(
        Column::builder()
            .gap(0.0)
            .children(vec![
                RenderNode::Row(
                    Row::builder()
                        .bg_color(BgColor::BgPanel)
                        .max_width(true)
                        .padding(8.0)
                        .children(vec![muted(status)])
                        .build(),
                ),
                RenderNode::Row(
                    Row::builder()
                        .gap(4.0)
                        .padding(4.0)
                        .max_width(true)
                        .align(Align::Fill)
                        .children(vec![
                            RenderNode::Input(
                                Input::builder()
                                    .id("ws-send-text")
                                    .value(st.ws_send_text.clone())
                                    .placeholder("Message to send…")
                                    .grow(true)
                                    .disabled(!st.ws_connected)
                                    .build(),
                            ),
                            btn(
                                "ws-send",
                                "Send",
                                st.ws_connected && !st.ws_send_text.is_empty(),
                                ButtonColor::Primary,
                            ),
                            RenderNode::Spacer(Spacer::builder().size(8.0).build()),
                        ])
                        .build(),
                ),
                RenderNode::Separator(Separator::plain()),
                RenderNode::Scroll(
                    Scroll::builder()
                        .id("ws-log-scroll")
                        .child(RenderNode::Column(
                            Column::builder().gap(2.0).children(log_rows).build(),
                        ))
                        .build(),
                ),
            ])
            .build(),
    )
}

// Phosphor (regular) arrows for the WebSocket log direction.
const ICON_ARROW_UP: &str = "\u{E08E}"; // ARROW_UP — sent
const ICON_ARROW_DOWN: &str = "\u{E03E}"; // ARROW_DOWN — received
const ICON_DOT: &str = "\u{E18A}"; // CIRCLE — system

/// One message-log line as a DataRow with a colour-coded direction arrow:
/// sent (↑, blue), received (↓, green), system (•, muted).
fn ws_log_row(idx: usize, entry: &WsLogEntry) -> RenderNode {
    let (glyph, color) = match entry.dir {
        WsDir::Sent => (ICON_ARROW_UP, Some("#89b4fa".to_string())),
        WsDir::Recv => (ICON_ARROW_DOWN, Some("#a6e3a1".to_string())),
        WsDir::System => (ICON_DOT, None),
    };
    RenderNode::DataRow(
        DataRow::builder()
            .row_id(format!("ws-log-{idx}"))
            .display_text(entry.text.clone())
            .leading_icon(
                DataRowIcon::builder()
                    .glyph(glyph)
                    .maybe_color(color)
                    .build(),
            )
            .build(),
    )
}

fn build_url_bar(st: &State) -> RenderNode {
    let method_options: Vec<SelectOption> = ["GET", "POST", "PUT", "PATCH", "DELETE", "WS", "WSS"]
        .iter()
        .map(|m| SelectOption::builder().value(*m).label(*m).build())
        .collect();

    RenderNode::Row(
        Row::builder()
            .gap(4.0)
            .padding(4.0)
            .max_width(true)
            .align(Align::Fill)
            .children(vec![
                RenderNode::Select(
                    Select::builder()
                        .id("method")
                        .value(st.method.clone())
                        .options(method_options)
                        .width(96.0)
                        .build(),
                ),
                RenderNode::Input(
                    Input::builder()
                        .id("url")
                        .value(st.url.clone())
                        .placeholder("https://api.example.com/endpoint")
                        .grow(true)
                        .required(true)
                        .build(),
                ),
                btn("clear", "Clear", true, ButtonColor::Danger),
                ws_or_send_button(st),
            ])
            .build(),
    )
}

/// True when the URL scheme is ws:// or wss://.
pub fn is_ws_url(url: &str) -> bool {
    let u = url.trim_start().to_ascii_lowercase();
    u.starts_with("ws://") || u.starts_with("wss://")
}

/// WebSocket mode is active when the method is WS/WSS or the URL already uses a
/// ws(s):// scheme (e.g. a pasted URL).
pub fn is_ws_mode(st: &State) -> bool {
    st.method == "WS" || st.method == "WSS" || is_ws_url(&st.url)
}

/// The URL to connect to, normalised to a ws(s):// scheme. If the URL already
/// has a ws(s):// scheme it's used as-is; otherwise the scheme is derived from
/// the selected method (WS → ws://, else wss://), replacing any http(s)://.
fn ws_connect_url(st: &State) -> String {
    let u = st.url.trim();
    if is_ws_url(u) {
        return u.to_string();
    }
    let scheme = if st.method == "WS" { "ws://" } else { "wss://" };
    let bare = u
        .strip_prefix("https://")
        .or_else(|| u.strip_prefix("http://"))
        .unwrap_or(u);
    format!("{scheme}{bare}")
}

/// Connect/Disconnect for ws(s):// URLs; ⚡ Send otherwise.
fn ws_or_send_button(st: &State) -> RenderNode {
    if is_ws_mode(st) {
        if st.ws_conn_id.is_some() {
            btn("ws-toggle", "Disconnect", true, ButtonColor::Danger)
        } else {
            btn(
                "ws-toggle",
                "Connect",
                !st.url.is_empty(),
                ButtonColor::Primary,
            )
        }
    } else {
        btn("send", "⚡ Send", !st.url.is_empty(), ButtonColor::Primary)
    }
}

fn build_req_tabs(st: &State) -> RenderNode {
    RenderNode::Tabs(
        Tabs::builder()
            .id("req-tabs")
            .headers(vec![
                "Params".to_string(),
                "Auth".to_string(),
                "Headers".to_string(),
                "Body".to_string(),
            ])
            .content_gap(0.0)
            .actions(vec![TabAction::builder()
                .id("export-curl")
                .icon(ICON_CODE)
                .tooltip("Export cURL")
                .build()])
            .children(vec![
                RenderNode::KeyValueList(
                    KeyValueList::builder()
                        .id("params")
                        .entries(to_entries(&st.params))
                        .add_label("Add param")
                        .build(),
                ),
                build_auth_panel(st),
                RenderNode::KeyValueList(
                    KeyValueList::builder()
                        .id("headers")
                        .entries(to_entries(&st.req_headers))
                        .add_label("Add header")
                        .build(),
                ),
                RenderNode::CodeEditor(
                    CodeEditor::builder()
                        .id("body")
                        .value(st.body.clone())
                        .rows(25)
                        .disabled(!is_body_method(&st.method))
                        .build(),
                ),
            ])
            .build(),
    )
}

fn build_auth_panel(st: &State) -> RenderNode {
    let type_opts = vec![
        SelectOption::builder()
            .value("none")
            .label("No Auth")
            .build(),
        SelectOption::builder()
            .value("bearer")
            .label("Bearer Token")
            .build(),
        SelectOption::builder()
            .value("basic")
            .label("Basic Auth")
            .build(),
        SelectOption::builder()
            .value("api-key")
            .label("API Key")
            .build(),
    ];

    let mut rows: Vec<RenderNode> = vec![RenderNode::Radio(
        Radio::builder()
            .id("auth-type")
            .label("Auth Type")
            .value(st.auth_type.clone())
            .options(type_opts)
            .build(),
    )];

    let password = |id: &str, label: &str, value: &str| {
        RenderNode::Input(
            Input::builder()
                .id(id)
                .label(label)
                .value(value.to_string())
                .password(true)
                .build(),
        )
    };
    let field = |id: &str, label: &str, value: &str, placeholder: &str| {
        RenderNode::Input(
            Input::builder()
                .id(id)
                .label(label)
                .value(value.to_string())
                .placeholder(placeholder.to_string())
                .build(),
        )
    };

    match st.auth_type.as_str() {
        "bearer" => rows.push(password("auth-token", "Token", &st.auth_token)),
        "basic" => {
            rows.push(field("auth-username", "Username", &st.auth_username, ""));
            rows.push(password("auth-password", "Password", &st.auth_password));
        }
        "api-key" => {
            rows.push(RenderNode::Radio(
                Radio::builder()
                    .id("auth-key-in")
                    .label("Add Key To")
                    .value(st.auth_key_in.clone())
                    .options(vec![
                        SelectOption::builder()
                            .value("header")
                            .label("Header")
                            .build(),
                        SelectOption::builder()
                            .value("query")
                            .label("Query Params")
                            .build(),
                    ])
                    .build(),
            ));
            let ph = if st.auth_key_in == "header" {
                "X-API-Key"
            } else {
                "api_key"
            };
            rows.push(field("auth-key-name", "Key Name", &st.auth_key_name, ph));
            rows.push(password("auth-key-value", "Value", &st.auth_key_value));
        }
        _ => {}
    }

    RenderNode::Column(Column::builder().gap(8.0).children(rows).build())
}

fn build_response_panel(resp: &ResponseState) -> RenderNode {
    let (color, status_label) = if resp.error.is_some() {
        ("#ef4444".to_string(), "Error".to_string())
    } else {
        (
            status_color(resp.status).to_string(),
            format!("{} {}", resp.status, status_text(resp.status)),
        )
    };

    let mut status_children: Vec<RenderNode> = vec![RenderNode::Badge(
        Badge::builder().label(status_label).color(color).build(),
    )];
    if let Some(ms) = resp.duration_ms {
        let t = if ms < 1000 {
            format!("{ms} ms")
        } else {
            format!("{:.2} s", ms as f64 / 1000.0)
        };
        status_children.push(muted(&t));
    }
    if resp.size_bytes > 0 {
        let s = if resp.size_bytes < 1024 {
            format!("{} B", resp.size_bytes)
        } else if resp.size_bytes < 1024 * 1024 {
            format!("{:.1} KB", resp.size_bytes as f64 / 1024.0)
        } else {
            format!("{:.1} MB", resp.size_bytes as f64 / (1024.0 * 1024.0))
        };
        status_children.push(muted(&s));
    }
    let status_row = RenderNode::Row(
        Row::builder()
            .bg_color(BgColor::BgPanel)
            .max_width(true)
            .height(20.0)
            .padding(10.0)
            .gap(8.0)
            .children(status_children)
            .build(),
    );

    if let Some(err) = &resp.error {
        return RenderNode::Column(
            Column::builder()
                .gap(0.0)
                .children(vec![
                    status_row,
                    RenderNode::Row(
                        Row::builder()
                            .bg_color(BgColor::Bg)
                            .max_width(true)
                            .padding(10.0)
                            .children(vec![text(err)])
                            .build(),
                    ),
                ])
                .build(),
        );
    }

    let pretty = match &resp.parsed_body {
        Some(val) => RenderNode::JsonTree(JsonTree::builder().value(val.clone()).build()),
        None => RenderNode::Code(
            Code::builder()
                .value(resp.body.clone())
                .language("text")
                .build(),
        ),
    };

    let header_rows: Vec<Vec<RenderNode>> = resp
        .headers
        .iter()
        .map(|h| vec![text(&h.key), muted(&h.value)])
        .collect();

    let resp_tabs = RenderNode::Tabs(
        Tabs::builder()
            .id("resp-tabs")
            .headers(vec![
                "Pretty".to_string(),
                "Raw".to_string(),
                "Headers".to_string(),
            ])
            .children(vec![
                pretty,
                RenderNode::Code(
                    Code::builder()
                        .value(resp.body.clone())
                        .language("json")
                        .build(),
                ),
                RenderNode::Table(
                    TableView::builder()
                        .headers(vec!["Header".to_string(), "Value".to_string()])
                        .rows(header_rows)
                        .build(),
                ),
            ])
            .build(),
    );

    RenderNode::Column(
        Column::builder()
            .gap(0.0)
            .children(vec![status_row, resp_tabs])
            .build(),
    )
}

// =============================================================================
// Async HTTP response handler
// =============================================================================

/// Called when the host delivers an async HTTP result via handle_event with
/// kind="http-response".
fn handle_http_response(st: &mut State, event: &UiEvent) {
    if st.pending_request_id.as_deref() != Some(event.widget_id.as_str()) {
        return;
    }

    let val: Value = match serde_json::from_str(&event.value) {
        Ok(v) => v,
        Err(e) => {
            st.loading = false;
            st.pending_request_id = None;
            st.response = Some(ResponseState {
                error: Some(format!("response parse error: {e}")),
                ..Default::default()
            });
            signals::emit_signal("http", &st.method, SignalStatus::Error, 0);
            return;
        }
    };

    let is_consent_pending = val
        .get("err")
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_str())
        .map(|c| c == "consent_pending")
        .unwrap_or_else(|| {
            val.get("err")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(|m| m.contains("waiting for user consent"))
                .unwrap_or(false)
        });
    if is_consent_pending {
        st.consent_pending = true;
        return;
    }

    st.loading = false;
    st.consent_pending = false;
    st.pending_request_id = None;
    st.resp_tab = "pretty".to_string();

    if let Some(ok) = val.get("ok") {
        let status = ok.get("status").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
        let body_raw = ok
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let headers: Vec<KvPair> = ok
            .get("headers")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|pair| {
                        let a = pair.as_array()?;
                        Some(KvPair {
                            key: a.first()?.as_str()?.to_string(),
                            value: a.get(1)?.as_str()?.to_string(),
                            enabled: true,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let duration_ms = ok.get("duration_ms").and_then(|v| v.as_u64());
        let size_bytes = body_raw.len();
        // Keep `body` as the exact raw payload so the Raw tab matches the
        // server response. The Pretty/JSON viewer uses `parsed_body`.
        let parsed_body = serde_json::from_str::<Value>(&body_raw).ok();
        st.response = Some(ResponseState {
            status,
            headers,
            body: body_raw,
            parsed_body,
            error: None,
            duration_ms,
            size_bytes,
        });
        // Status-bar signal: request method + HTTP status + latency; >=400 is error.
        let sig_status = if status >= 400 {
            SignalStatus::Error
        } else {
            SignalStatus::Ready
        };
        let latency = duration_ms
            .map(|ms| format!(" · {ms} ms"))
            .unwrap_or_default();
        signals::emit_signal(
            "http",
            &format!("{} {status}{latency}", st.method),
            sig_status,
            0,
        );
    } else if let Some(err) = val.get("err") {
        let message = err
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error")
            .to_string();
        st.response = Some(ResponseState {
            error: Some(message),
            ..Default::default()
        });
        signals::emit_signal("http", &st.method, SignalStatus::Error, 0);
    } else {
        // Payload had neither `ok` nor `err`: still clear the sticky Loading.
        signals::emit_signal("http", &st.method, SignalStatus::Error, 0);
    }
}

// =============================================================================
// WebSocket handlers
// =============================================================================

/// Human-readable byte count for the status bar.
fn fmt_bytes(n: usize) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    }
}

/// Push the WebSocket status-bar signal: total bytes sent / received.
fn emit_ws_signal(st: &State, status: SignalStatus) {
    let value = format!(
        "↑{} ↓{}",
        fmt_bytes(st.ws_bytes_sent),
        fmt_bytes(st.ws_bytes_recv)
    );
    signals::emit_signal("ws", &value, status, 0);
}

/// Append a log line, capping the log so it can't grow without bound.
fn ws_log(st: &mut State, dir: WsDir, text: impl Into<String>) {
    const MAX: usize = 500;
    st.ws_log.push(WsLogEntry {
        dir,
        text: text.into(),
    });
    if st.ws_log.len() > MAX {
        let excess = st.ws_log.len() - MAX;
        st.ws_log.drain(0..excess);
    }
}

/// Connect headers: enabled custom request headers, plus bearer auth.
fn ws_headers(st: &State) -> Vec<(String, String)> {
    let mut headers: Vec<(String, String)> = st
        .req_headers
        .iter()
        .filter(|h| h.enabled && !h.key.is_empty())
        .map(|h| (h.key.clone(), h.value.clone()))
        .collect();
    if st.auth_type == "bearer" && !st.auth_token.is_empty() {
        headers.push((
            "Authorization".to_string(),
            format!("Bearer {}", st.auth_token),
        ));
    }
    headers
}

fn ws_toggle(st: &mut State) {
    if st.ws_conn_id.is_some() {
        ws_disconnect(st);
    } else {
        ws_connect(st);
    }
}

fn ws_connect(st: &mut State) {
    if st.url.is_empty() {
        return;
    }
    let url = ws_connect_url(st);
    let headers = ws_headers(st);
    match websocket::connect(&url, &headers) {
        Ok(id) => {
            st.ws_conn_id = Some(id);
            st.ws_connected = false;
            st.ws_bytes_sent = 0;
            st.ws_bytes_recv = 0;
            ws_log(st, WsDir::System, format!("connecting to {url}…"));
            emit_ws_signal(st, SignalStatus::Loading);
        }
        Err(e) => ws_log(st, WsDir::System, format!("connect failed: {}", e.message)),
    }
}

fn ws_disconnect(st: &mut State) {
    if let Some(id) = st.ws_conn_id.take() {
        websocket::close(&id);
    }
    st.ws_connected = false;
    ws_log(st, WsDir::System, "disconnected");
}

fn ws_send(st: &mut State) {
    let Some(id) = st.ws_conn_id.clone() else {
        return;
    };
    let text = st.ws_send_text.clone();
    if text.is_empty() {
        return;
    }
    match websocket::send_text(&id, &text) {
        Ok(()) => {
            st.ws_bytes_sent += text.len();
            ws_log(st, WsDir::Sent, text);
            st.ws_send_text.clear();
            emit_ws_signal(st, SignalStatus::Ready);
        }
        Err(e) => ws_log(st, WsDir::System, format!("send failed: {}", e.message)),
    }
}

/// Fold a host WebSocket event (ws-open/message/error/close) into state.
fn handle_ws_event(st: &mut State, event: &UiEvent) {
    // Only events for the current connection matter (ignore a stale one).
    if st.ws_conn_id.as_deref() != Some(event.widget_id.as_str()) {
        return;
    }
    match event.kind.as_str() {
        "ws-open" => {
            st.ws_connected = true;
            ws_log(st, WsDir::System, "connected");
            emit_ws_signal(st, SignalStatus::Ready);
        }
        "ws-message" => {
            let v: Value = serde_json::from_str(&event.value).unwrap_or(Value::Null);
            if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
                st.ws_bytes_recv += t.len();
                ws_log(st, WsDir::Recv, t.to_string());
            } else if let Some(hex) = v.get("binary").and_then(|b| b.as_str()) {
                let len = v.get("len").and_then(|l| l.as_u64()).unwrap_or(0);
                st.ws_bytes_recv += len as usize;
                ws_log(st, WsDir::Recv, format!("<binary {len} bytes> {hex}"));
            }
            emit_ws_signal(st, SignalStatus::Ready);
        }
        "ws-error" => {
            // Errors are terminal (the host task ends), so reset like a close.
            st.ws_connected = false;
            st.ws_conn_id = None;
            let msg = event.value.clone();
            ws_log(st, WsDir::System, format!("error: {msg}"));
            emit_ws_signal(st, SignalStatus::Error);
            // Release the host-side connection entry.
            websocket::close(&event.widget_id);
        }
        "ws-close" => {
            let v: Value = serde_json::from_str(&event.value).unwrap_or(Value::Null);
            let code = v.get("code").and_then(|c| c.as_u64()).unwrap_or(0);
            let reason = v
                .get("reason")
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_string();
            st.ws_connected = false;
            st.ws_conn_id = None;
            let msg = if reason.is_empty() {
                format!("closed ({code})")
            } else {
                format!("closed ({code}): {reason}")
            };
            ws_log(st, WsDir::System, msg);
            // Keep the final byte totals visible, but mark the socket idle.
            emit_ws_signal(st, SignalStatus::Ready);
            // Release the host-side connection entry (server-initiated close).
            websocket::close(&event.widget_id);
        }
        _ => {}
    }
}

// =============================================================================
// Event → state mutations
// =============================================================================

pub fn apply_event(st: &mut State, event: &UiEvent) {
    if event.kind == "http-response" {
        handle_http_response(st, event);
        return;
    }
    // WebSocket lifecycle/message events are keyed by connection id, not a
    // known widget id, so route them by kind.
    if event.kind.starts_with("ws-") {
        handle_ws_event(st, event);
        return;
    }

    match event.widget_id.as_str() {
        "ws-toggle" => ws_toggle(st),
        "ws-send-text" => {
            st.ws_send_text = parse_str(&event.value);
            // Enter in the box sends.
            if event.kind == "submit" {
                ws_send(st);
            }
        }
        "ws-send" => ws_send(st),
        "request-name" => st.request_name = parse_str(&event.value),

        "method" => st.method = parse_str(&event.value),
        "url" => {
            let raw = parse_str(&event.value);
            parse_url_into_state(st, raw);
            // Typing/pasting a ws(s):// URL switches the method to WS/WSS so the
            // UI enters WebSocket mode.
            let scheme = st.url.trim_start().to_ascii_lowercase();
            if scheme.starts_with("wss://") {
                st.method = "WSS".to_string();
            } else if scheme.starts_with("ws://") {
                st.method = "WS".to_string();
            } else if st.ws_conn_id.is_some() && !is_ws_url(&st.url) {
                // Navigated away from a WebSocket endpoint while connected —
                // close the socket so it isn't orphaned.
                ws_disconnect(st);
            }
        }

        "req-tabs" if parse_str(&event.value) == "Body" && !is_body_method(&st.method) => {
            st.method = "POST".to_string();
        }

        "params" => st.params = parse_kv_list(&event.value),
        "headers" => st.req_headers = parse_kv_list(&event.value),
        "body" => st.body = parse_str(&event.value),

        "auth-type" => st.auth_type = parse_str(&event.value),
        "auth-token" => st.auth_token = parse_str(&event.value),
        "auth-username" => st.auth_username = parse_str(&event.value),
        "auth-password" => st.auth_password = parse_str(&event.value),
        "auth-key-name" => st.auth_key_name = parse_str(&event.value),
        "auth-key-value" => st.auth_key_value = parse_str(&event.value),
        "auth-key-in" => st.auth_key_in = parse_str(&event.value),

        "send" if !st.url.is_empty() => {
            let req = build_request(st);
            let request_id = http_client::submit(&req);
            st.pending_request_id = Some(request_id);
            st.loading = true;
            st.response = None;
            // Push a "requesting" signal (method only); the response overwrites
            // it with method + status + latency.
            signals::emit_signal("http", &st.method, SignalStatus::Loading, 0);
        }

        _ => {}
    }
}
