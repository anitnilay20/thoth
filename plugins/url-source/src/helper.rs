use serde_json::Value;

use crate::{
    bindings::{
        exports::thoth::plugin::{data_source::ConfigEntry, ui_component::UiOutput},
        thoth::plugin::http_client::PluginError,
    },
    KvPair, State,
};

pub fn ce(name: &str, description: &str, required: bool, default: &str) -> ConfigEntry {
    ConfigEntry {
        name: name.to_string(),
        description: description.to_string(),
        required,
        value: default.to_string(),
    }
}

pub fn plugin_err(code: u32, message: impl Into<String>) -> PluginError {
    PluginError {
        code,
        message: message.into(),
    }
}

pub fn ui_out(node: thoth_plugin_sdk::render_node::RenderNode) -> UiOutput {
    UiOutput {
        node_json: serde_json::to_string(&node)
            .expect("RenderNode serialization should be infallible"),
        height_hint: 0,
    }
}

pub fn parse_str(s: &str) -> String {
    serde_json::from_str::<String>(s).unwrap_or_else(|_| s.to_string())
}

pub fn parse_kv_list(s: &str) -> Vec<KvPair> {
    serde_json::from_str(s).unwrap_or_default()
}

/// True when the form holds a meaningful request (so we shouldn't overwrite it
/// in place, e.g. when importing a cURL — open a new tab instead).
pub fn request_is_non_empty(st: &State) -> bool {
    !st.url.is_empty()
        || !st.body.is_empty()
        || st.params.iter().any(|p| !p.key.is_empty())
        || st.req_headers.iter().any(|h| !h.key.is_empty())
}

/// Replace `{{var}}` placeholders in `input` using enabled `vars` (#128).
pub fn substitute_env(input: &str, vars: &[KvPair]) -> String {
    let mut out = input.to_string();
    for v in vars {
        if v.enabled && !v.key.trim().is_empty() {
            let needle = format!("{{{{{}}}}}", v.key.trim());
            out = out.replace(&needle, &v.value);
        }
    }
    out
}

/// Compute a simple line-by-line diff between two response bodies (#129).
pub fn compute_line_diff(left: &str, right: &str) -> String {
    let a: Vec<&str> = left.lines().collect();
    let b: Vec<&str> = right.lines().collect();
    let mut out = Vec::new();
    let max_len = a.len().max(b.len());
    for i in 0..max_len {
        match (a.get(i), b.get(i)) {
            (Some(la), Some(rb)) if la == rb => out.push(format!("  {la}")),
            (Some(la), Some(rb)) => {
                out.push(format!("- {la}"));
                out.push(format!("+ {rb}"));
            }
            (Some(la), None) => out.push(format!("- {la}")),
            (None, Some(rb)) => out.push(format!("+ {rb}")),
            (None, None) => {}
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_enabled_env_vars_only() {
        let vars = vec![
            KvPair {
                key: "base_url".into(),
                value: "https://api.example.com".into(),
                enabled: true,
            },
            KvPair {
                key: "token".into(),
                value: "secret".into(),
                enabled: false,
            },
        ];
        assert_eq!(
            substitute_env("{{base_url}}/v1?t={{token}}", &vars),
            "https://api.example.com/v1?t={{token}}"
        );
    }

    #[test]
    fn computes_line_diff_markers() {
        let diff = compute_line_diff("a\nb\nc", "a\nx\nc\nd");
        assert_eq!(diff, "  a\n- b\n+ x\n  c\n+ d");
    }
}

pub fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

pub fn is_body_method(method: &str) -> bool {
    matches!(method.to_uppercase().as_str(), "POST" | "PUT" | "PATCH")
}

pub fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many",
        500 => "Server Error",
        502 => "Bad Gateway",
        503 => "Unavailable",
        _ => "",
    }
}

pub fn normalise_array(v: Value) -> Value {
    match v {
        Value::Array(_) => v,
        obj => Value::Array(vec![obj]),
    }
}

pub fn type_hint(v: &Value) -> String {
    match v {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Object(_) => "object",
        Value::Array(_) => "array",
        Value::Null => "string",
    }
    .to_string()
}
// ── URL parsing ──────────────────────────────────────────────────────────────

/// When the user types or pastes a URL that contains a `?` query string,
/// split it: store the bare URL in `st.url` and merge the query params into
/// `st.params`.  Existing params that don't appear in the new URL are kept;
/// params present in the URL overwrite same-key existing entries.
pub fn parse_url_into_state(st: &mut State, raw: String) {
    // Only trigger when a `?` is present — plain typing shouldn't re-parse.
    let Some(q_pos) = raw.find('?') else {
        st.url = raw;
        return;
    };

    let base = raw[..q_pos].to_string();
    let query = &raw[q_pos + 1..];

    // Decode percent-encoding for display (best-effort, no external deps).
    let decode = |s: &str| percent_decode(s);

    let mut parsed: Vec<KvPair> = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| {
            if let Some(eq) = pair.find('=') {
                KvPair {
                    key: decode(&pair[..eq]),
                    value: decode(&pair[eq + 1..]),
                    enabled: true,
                }
            } else {
                KvPair {
                    key: decode(pair),
                    value: String::new(),
                    enabled: true,
                }
            }
        })
        .collect();

    // Merge: overwrite existing entries with the same key, append new ones.
    for new_param in &parsed {
        if let Some(existing) = st.params.iter_mut().find(|p| p.key == new_param.key) {
            existing.value = new_param.value.clone();
        }
    }
    // Append params whose keys weren't already in st.params.
    let existing_keys: std::collections::HashSet<&str> =
        st.params.iter().map(|p| p.key.as_str()).collect();
    parsed.retain(|p| !existing_keys.contains(p.key.as_str()));
    st.params.extend(parsed);

    st.url = base;
}

/// Minimal percent-decode: replace `%XX` sequences and `+` with spaces.
pub fn percent_decode(s: &str) -> String {
    let mut out: Vec<u8> = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(h << 4 | l);
                i += 3;
            } else {
                out.push(b'%');
                i += 1;
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
