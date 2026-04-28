use crate::{
    bindings::{exports::thoth::plugin::ui_component::UiEvent, thoth::plugin::http_client},
    helper::{
        is_body_method, parse_kv_list, parse_str, parse_url_into_state, status_color, status_text,
    },
    http::build_request,
    KvPair, ResponseState, State,
};
use serde_json::{json, Value};

pub fn build_ui(st: &State) -> Value {
    // Two-column split: request form | response
    // (saved-requests panel lives in the host sidebar via render_sidebar)
    json!({
        "type": "column",
        "gap": 0,
        "children": [
            build_url_bar(st),
            {"type": "separator"},
            {
                "type": "split",
                "gap": 0,
                "separator": true,
                "children": [
                    build_request_column(st),
                    build_response_column(st)
                ]
            }
        ]
    })
}

fn method_badge_color(method: &str) -> &'static str {
    match method {
        "GET" => "blue",
        "POST" => "green",
        "PUT" => "orange",
        "PATCH" => "orange",
        "DELETE" => "red",
        "HEAD" => "purple",
        "OPTIONS" => "purple",
        _ => "gray",
    }
}

// egui_phosphor::regular::ARROW_SQUARE_OUT = export, ARROW_SQUARE_IN = import
const ICON_EXPORT: &str = "\u{E5DE}";
const ICON_IMPORT: &str = "\u{E5DC}";

/// Rendered by the host in its sidebar panel.
pub fn build_sidebar(st: &State) -> Value {
    let list_items: Vec<Value> = st
        .saved_requests
        .iter()
        .map(|req| {
            json!({
                "title": req.name,
                "description": req.url,
                "badge": {
                    "text": req.method,
                    "color": method_badge_color(&req.method)
                },
                "actions": [{"icon": "x", "tooltip": "Delete"}]
            })
        })
        .collect();

    let curl_command = crate::build_curl_command(st);

    json!({
        "type": "column",
        "gap": 0,
        "children": [
            {
                "type": "row",
                "gap": 4,
                "align": "fill",
                "padding": 6,
                "children": [
                    {
                        "type": "text-input",
                        "id": "request-name",
                        "value": st.request_name,
                        "placeholder": "Request name",
                        "label": "",
                        "grow": true
                    },
                    btn_elevated("save", "Save", !st.url.is_empty(), "Default")
                ]
            },
            {"type": "separator"},
            {
                "type": "list",
                "id": "saved-requests",
                "items": list_items,
                "empty-label": "No saved requests"
            },
            // Export modal
            {
                "type": "modal",
                "id": "export-modal",
                "title": "Export cURL",
                "open": st.show_export_modal,
                "close-id": "close-export",
                "width-pct": 0.7,
                "height-pct": 0.7,
                "children": [
                    {
                        "type": "text-input",
                        "id": "curl-output",
                        "value": curl_command,
                        "label": "cURL command",
                        "placeholder": "",
                        "multiline": true,
                        "rows": 10
                    },
                    {
                        "type": "button",
                        "id": "",
                        "copy": curl_command,
                        "props": {
                            "label": "Copy to Clipboard",
                            "button-type": "Elevated",
                            "color": "Default",
                            "enabled": true
                        }
                    }
                ]
            },
            // Import modal
            {
                "type": "modal",
                "id": "import-modal",
                "title": "Import cURL",
                "open": st.show_import_modal,
                "close-id": "close-import",
                "width-pct": 0.7,
                "height-pct": 0.7,
                "children": [
                    {
                        "type": "text-input",
                        "id": "curl-import-input",
                        "value": st.curl_import_input,
                        "placeholder": "Paste cURL command here…",
                        "label": "cURL command",
                        "multiline": true,
                        "rows": 10
                    },
                    btn_elevated("curl-import-submit", "Import", !st.curl_import_input.is_empty(), "Default")
                ]
            },
            // Footer pinned at bottom
            {
                "type": "footer",
                "padding": 8,
                "gap": 0,
                "children": [
                    {"type": "separator"},
                    {
                        "type": "row",
                        "gap": 4,
                        "padding": 4,
                        "children": [
                            btn_text_icon("export-curl", "Export cURL", ICON_EXPORT),
                            btn_text_icon("import-curl", "Import cURL", ICON_IMPORT)
                        ]
                    }
                ]
            }
        ]
    })
}

fn build_request_column(st: &State) -> Value {
    json!({
        "type": "scroll",
        "id": "request_column",
        "child": {
            "type": "column",
            "gap": 6,
            "children": [build_req_tabs(st)]
        }
    })
}

fn build_response_column(st: &State) -> Value {
    if st.loading {
        let label = if st.consent_pending {
            "Waiting for consent approval…"
        } else {
            "Sending request…"
        };
        return json!({
            "type":      "row",
            "bg-color":  "mantle",
            "max-width": true,
            "padding":   10,
            "gap":       8,
            "children": [
                {"type": "spinner", "size": 14},
                {"type": "text", "value": label, "muted": true}
            ]
        });
    }

    if let Some(resp) = &st.response {
        json!({
            "type": "scroll",
            "id": "response_column",
            "child": build_response_panel(st, resp)
        })
    } else {
        json!({
            "type": "row",
            "bg-color": "mantle",
            "max-width": true,
            "height": 20,
            "padding": 10,
            "children": [{
                "type": "text",
                "value": "Send a request to see the response here.",
            }]
        })
    }
}

fn build_url_bar(st: &State) -> Value {
    // Method pill buttons
    let method_btns: Vec<Value> = ["GET", "POST", "PUT", "PATCH", "DELETE"]
        .iter()
        .map(|m| {
            let id = format!("method-btn-{}", m.to_lowercase());
            let selected = st.method == *m;
            if selected {
                btn_elevated(&id, m, true, "Secondary")
            } else {
                btn_text(&id, m, true)
            }
        })
        .collect();

    // Visual order: [MethodButtons | URL(grow) | Send | Save]
    // Fill layout: prefix items render LTR, then RTL sub-layout for suffix+grow.
    json!({
        "type":  "row",
        "gap":   4,
        "align": "fill",
        "padding": 4,
        "children": [
            {
                "type": "row",
                "gap": 2,
                "children": method_btns
            },
            {
                "type":        "text-input",
                "id":          "url",
                "value":       st.url,
                "placeholder": "https://api.example.com/endpoint",
                "label":       "",
                "grow":        true,
                "required":    true
            },
            btn_elevated("clear", "Clear", true, "Danger"),
            btn_elevated("send", "⚡ Send", !st.url.is_empty(), "Primary"),
        ]
    })
}

fn build_req_tabs(st: &State) -> Value {
    // The `tabs` DSL node embeds all tab content as children.
    // The host tracks the active tab index in egui memory — the plugin no longer
    // needs to manage `active_tab` for rendering.  A "change" event is emitted
    // with the selected header label so the plugin can still react (e.g. Body
    // tab → auto-promote method to POST).
    json!({
        "type":   "tabs",
        "id":     "req-tabs",
        "header": ["Params", "Auth", "Headers", "Body"],
        "children": [
            // ── Params ──────────────────────────────────────────────────────
            {
                "type":      "key-value-list",
                "id":        "params",
                "label":     "",
                "entries":   st.params,
                "add-label": "Add param"
            },
            // ── Auth ────────────────────────────────────────────────────────
            build_auth_panel(st),
            // ── Headers ─────────────────────────────────────────────────────
            {
                "type":      "key-value-list",
                "id":        "headers",
                "label":     "",
                "entries":   st.req_headers,
                "add-label": "Add header"
            },
            // ── Body ────────────────────────────────────────────────────────
            {
                "type":     "code-editor",
                "id":       "body",
                "value":    st.body,
                "disabled": !is_body_method(&st.method)
            }
        ]
    })
}

fn build_auth_panel(st: &State) -> Value {
    let type_opts: Vec<Value> = vec![
        json!({"value": "none",    "label": "No Auth"}),
        json!({"value": "bearer",  "label": "Bearer Token"}),
        json!({"value": "basic",   "label": "Basic Auth"}),
        json!({"value": "api-key", "label": "API Key"}),
    ];

    let mut rows: Vec<Value> = vec![json!({
        "type":    "radio",
        "id":      "auth-type",
        "label":   "Auth Type",
        "value":   st.auth_type,
        "options": type_opts
    })];

    match st.auth_type.as_str() {
        "bearer" => rows.push(json!({
            "type":  "password-input",
            "id":    "auth-token",
            "label": "Token",
            "value": st.auth_token
        })),

        "basic" => {
            rows.push(json!({
                "type":  "text-input",
                "id":    "auth-username",
                "label": "Username",
                "value": st.auth_username
            }));
            rows.push(json!({
                "type":  "password-input",
                "id":    "auth-password",
                "label": "Password",
                "value": st.auth_password
            }));
        }

        "api-key" => {
            rows.push(json!({
                "type":    "radio",
                "id":      "auth-key-in",
                "label":   "Add Key To",
                "value":   st.auth_key_in,
                "options": [
                    {"value": "header", "label": "Header"},
                    {"value": "query",  "label": "Query Params"}
                ]
            }));
            rows.push(json!({
                "type":        "text-input",
                "id":          "auth-key-name",
                "label":       "Key Name",
                "value":       st.auth_key_name,
                "placeholder": if st.auth_key_in == "header" { "X-API-Key" } else { "api_key" }
            }));
            rows.push(json!({
                "type":  "password-input",
                "id":    "auth-key-value",
                "label": "Value",
                "value": st.auth_key_value
            }));
        }

        _ => {} // none
    }

    json!({"type": "column", "gap": 8, "children": rows})
}

fn build_response_panel(_st: &State, resp: &ResponseState) -> Value {
    let is_error = resp.error.is_some();
    let (color, status_label) = if is_error {
        ("#ef4444", "Error".to_string())
    } else {
        (
            status_color(resp.status),
            format!("{} {}", resp.status, status_text(resp.status)),
        )
    };

    // Status bar: badge + optional time + size
    let mut status_children = vec![json!({"type": "badge", "label": status_label, "color": color})];
    if let Some(ms) = resp.duration_ms {
        let time_label = if ms < 1000 {
            format!("{ms} ms")
        } else {
            format!("{:.2} s", ms as f64 / 1000.0)
        };
        status_children.push(json!({"type": "text", "value": time_label, "muted": true}));
    }
    if resp.size_bytes > 0 {
        let size_label = if resp.size_bytes < 1024 {
            format!("{} B", resp.size_bytes)
        } else if resp.size_bytes < 1024 * 1024 {
            format!("{:.1} KB", resp.size_bytes as f64 / 1024.0)
        } else {
            format!("{:.1} MB", resp.size_bytes as f64 / (1024.0 * 1024.0))
        };
        status_children.push(json!({"type": "text", "value": size_label, "muted": true}));
    }
    let status_row = json!({
        "type":      "row",
        "bg-color":  "mantle",
        "max-width": true,
        "height":    20,
        "padding":   10,
        "gap":       8,
        "children":  status_children
    });

    // Error: show the message wrapped in a padded text block, no response tabs.
    if let Some(err) = &resp.error {
        return json!({
            "type": "column",
            "gap":  0,
            "children": [
                status_row,
                {
                    "type":      "row",
                    "bg-color":  "base",
                    "max-width": true,
                    "padding":   10,
                    "children": [{
                        "type":  "text",
                        "value": err,
                        "muted": false
                    }]
                }
            ]
        });
    }

    // Try to parse body as JSON once so both Pretty and Raw can use it.
    let pretty_node = match serde_json::from_str::<Value>(&resp.body) {
        Ok(val) => json!({"type": "json-tree", "value": val}),
        Err(_) => json!({"type": "code", "value": resp.body, "language": "text"}),
    };

    let resp_tabs = json!({
        "type":   "tabs",
        "id":     "resp-tabs",
        "header": ["Pretty", "Raw", "Headers"],
        "children": [
            pretty_node,
            {"type": "code", "value": resp.body, "language": "json"},
            {
                "type":    "table",
                "headers": ["Header", "Value"],
                "rows": resp.headers.iter().map(|h| {
                    vec![
                        json!({"type": "text", "value": h.key,   "muted": false}),
                        json!({"type": "text", "value": h.value, "muted": true }),
                    ]
                }).collect::<Vec<_>>()
            }
        ]
    });

    json!({
        "type": "column",
        "gap":  0,
        "children": [
            status_row,
            resp_tabs
        ]
    })
}

// =============================================================================
// Async HTTP response handler
// =============================================================================

/// Called when the host delivers an async HTTP result via handle_event with
/// kind="http-response".  value is JSON:
///   {"ok":{"status":200,"headers":[["k","v"]],"body":"..."}}
///   {"err":{"code":1,"message":"..."}}
fn handle_http_response(st: &mut State, event: &UiEvent) {
    let val: Value = match serde_json::from_str(&event.value) {
        Ok(v) => v,
        Err(e) => {
            st.loading = false;
            st.pending_request_id = None;
            st.response = Some(ResponseState {
                error: Some(format!("response parse error: {e}")),
                ..Default::default()
            });
            return;
        }
    };

    // Consent-pending sentinel: keep loading state and spinner visible so the
    // user knows to action the consent popup rather than seeing a bare error.
    if let Some(msg) = val
        .get("err")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
    {
        if msg.contains("waiting for user consent") {
            st.consent_pending = true;
            // loading stays true, pending_request_id stays set
            return;
        }
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
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let duration_ms = ok.get("duration_ms").and_then(|v| v.as_u64());
        let size_bytes = body_raw.len();
        // Pretty-print if the body is valid JSON.
        let body = serde_json::from_str::<Value>(&body_raw)
            .map(|v| serde_json::to_string_pretty(&v).unwrap_or(body_raw.clone()))
            .unwrap_or(body_raw);
        st.response = Some(ResponseState {
            status,
            headers,
            body,
            error: None,
            duration_ms,
            size_bytes,
        });
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
    }
}

// =============================================================================
// Event → state mutations
// =============================================================================

pub fn apply_event(st: &mut State, event: &UiEvent) {
    // HTTP response delivered asynchronously by the host.
    if event.kind == "http-response" {
        handle_http_response(st, event);
        return;
    }

    match event.widget_id.as_str() {
        "request-name" => st.request_name = parse_str(&event.value),

        "method-btn-get" => st.method = "GET".to_string(),
        "method-btn-post" => st.method = "POST".to_string(),
        "method-btn-put" => st.method = "PUT".to_string(),
        "method-btn-delete" => st.method = "DELETE".to_string(),
        "method-btn-patch" => st.method = "PATCH".to_string(),
        "method" => st.method = parse_str(&event.value),
        "url" => {
            let raw = parse_str(&event.value);
            parse_url_into_state(st, raw);
        }

        // req-tabs emits a "change" event with the header label when switched.
        // We only need to react to "Body" — auto-promote method so the editor
        // becomes enabled.
        "req-tabs" => {
            if parse_str(&event.value) == "Body" && !is_body_method(&st.method) {
                st.method = "POST".to_string();
            }
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

        "send" => {
            if !st.url.is_empty() {
                let req = build_request(st);
                let request_id = http_client::submit(&req);
                st.pending_request_id = Some(request_id);
                st.loading = true;
                st.response = None; // clear previous response while loading
            }
        }

        _ => {}
    }
}

// ── Button helpers ────────────────────────────────────────────────────────────
// Maps the old variant/enabled pattern to the new ButtonProps JSON shape.

/// Elevated (filled) button. Use for primary actions and active tab state.
fn btn_elevated(id: &str, label: &str, enabled: bool, color: &str) -> Value {
    json!({
        "type": "button",
        "id":   id,
        "props": {
            "label":       label,
            "button-type": "Elevated",
            "color":       color,
            "enabled":     enabled
        }
    })
}

/// Text (ghost) button. Use for inactive method pill buttons.
fn btn_text(id: &str, label: &str, enabled: bool) -> Value {
    json!({
        "type": "button",
        "id":   id,
        "props": {
            "label":       label,
            "button-type": "Text",
            "color":       "Default",
            "enabled":     enabled
        }
    })
}

/// Text button with a phosphor icon.
fn btn_text_icon(id: &str, label: &str, icon: &str) -> Value {
    json!({
        "type": "button",
        "id":   id,
        "props": {
            "label":       label,
            "button-type": "Text",
            "color":       "Default",
            "enabled":     true,
            "icon":        icon
        }
    })
}
