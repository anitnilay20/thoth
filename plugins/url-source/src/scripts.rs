//! Pre/post-request script runner for `url-source` (#130).
//!
//! Runs a lightweight, deterministic line-oriented script DSL inside the WASM
//! sandbox before sending a request or after receiving a response:
//!
//! Pre-request directives (lines starting with `#` or `//` are comments):
//! - `set header <Name> <Value>`
//! - `set param <Name> <Value>`
//! - `set body <Text>`
//! - `append body <Text>`
//!
//! Post-request directives:
//! - `select <$.path.to.node>`  (replaces displayed body with extracted JSON)
//! - `assert status == <code>`
//! - `assert contains <substring>`

use serde_json::Value;

use crate::{KvPair, State};

fn upsert_kv(list: &mut Vec<KvPair>, key: &str, value: &str) {
    if let Some(existing) = list.iter_mut().find(|p| p.key.eq_ignore_ascii_case(key)) {
        existing.value = value.to_string();
        existing.enabled = true;
    } else {
        list.push(KvPair {
            key: key.to_string(),
            value: value.to_string(),
            enabled: true,
        });
    }
}

/// Run `st.pre_script` against `st` before building the outgoing HTTP request.
pub(crate) fn run_pre_script(st: &mut State) {
    let script = st.pre_script.clone();
    for raw_line in script.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("set header ") {
            if let Some((k, v)) = rest.trim().split_once(char::is_whitespace) {
                upsert_kv(&mut st.req_headers, k.trim(), v.trim());
            }
        } else if let Some(rest) = line.strip_prefix("set param ") {
            if let Some((k, v)) = rest.trim().split_once(char::is_whitespace) {
                upsert_kv(&mut st.params, k.trim(), v.trim());
            }
        } else if let Some(rest) = line.strip_prefix("set body ") {
            st.body = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("append body ") {
            st.body.push_str(rest);
        }
    }
}

/// Extract a `$.a.b.0` JSON path from `root`.
fn extract_json_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let trimmed = path.trim().strip_prefix('$').unwrap_or(path.trim());
    let mut cur = root;
    for seg in trimmed.split('.').filter(|s| !s.is_empty()) {
        if let Ok(idx) = seg.parse::<usize>() {
            cur = cur.get(idx)?;
        } else {
            cur = cur.get(seg)?;
        }
    }
    Some(cur)
}

/// Run `st.post_script` against the received `status` and `body`.
/// Returns `Err(assertion_failure)` if an `assert` directive fails.
pub(crate) fn run_post_script(
    script: &str,
    status: u16,
    body: &mut String,
) -> Result<(), String> {
    for raw_line in script.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if let Some(path) = line.strip_prefix("select ") {
            if let Ok(parsed) = serde_json::from_str::<Value>(body) {
                if let Some(selected) = extract_json_path(&parsed, path) {
                    if let Ok(pretty) = serde_json::to_string_pretty(selected) {
                        *body = pretty;
                    }
                }
            }
        } else if let Some(expected) = line.strip_prefix("assert status == ") {
            if let Ok(code) = expected.trim().parse::<u16>() {
                if status != code {
                    return Err(format!("assertion failed: expected status {code}, got {status}"));
                }
            }
        } else if let Some(needle) = line.strip_prefix("assert contains ") {
            if !body.contains(needle.trim()) {
                return Err(format!(
                    "assertion failed: response body does not contain '{}'",
                    needle.trim()
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pre_script_updates_headers_params_and_body() {
        let mut st = State::fresh();
        st.pre_script = "\
# add auth and query
set header X-Trace-Id abc-123
set param limit 50
set body {\"ok\":true}
"
        .to_string();
        run_pre_script(&mut st);
        assert_eq!(st.req_headers[0].key, "X-Trace-Id");
        assert_eq!(st.req_headers[0].value, "abc-123");
        assert_eq!(st.params[0].key, "limit");
        assert_eq!(st.params[0].value, "50");
        assert_eq!(st.body, "{\"ok\":true}");
    }

    #[test]
    fn post_script_selects_subtree_and_checks_assertions() {
        let mut body = r#"{"data":{"items":[{"id":1},{"id":2}]}}"#.to_string();
        let script = "assert status == 200\nselect $.data.items.1\nassert contains 2";
        assert!(run_post_script(script, 200, &mut body).is_ok());
        assert!(body.contains("\"id\": 2"));

        let mut body2 = r#"{"ok":false}"#.to_string();
        assert!(run_post_script("assert status == 201", 200, &mut body2).is_err());
    }
}
