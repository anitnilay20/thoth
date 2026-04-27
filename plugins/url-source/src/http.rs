use base64::Engine as _;

use crate::helper::plugin_err;
use crate::{
    bindings::thoth::plugin::http_client::{self, HttpRequest, PluginError},
    helper::{is_body_method, pct_encode, status_text},
    KvPair, State,
};

pub fn http_fetch(st: &State) -> Result<Vec<u8>, PluginError> {
    let req = build_request(st);
    let resp = http_client::fetch(&req)
        .map_err(|e| plugin_err(1, format!("HTTP error: {}", e.message)))?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(plugin_err(
            resp.status as u32,
            format!("HTTP {}: {}", resp.status, status_text(resp.status)),
        ));
    }
    Ok(resp.body)
}

/// Build an `HttpRequest` from the current plugin state.
pub fn build_request(st: &State) -> HttpRequest {
    // URL + query params
    let url = {
        let active: Vec<&KvPair> = st.params.iter().filter(|p| !p.key.is_empty()).collect();
        if active.is_empty() {
            // Also handle api-key-in-query auth
            if st.auth_type == "api-key"
                && st.auth_key_in == "query"
                && !st.auth_key_name.is_empty()
            {
                let sep = if st.url.contains('?') { '&' } else { '?' };
                format!(
                    "{}{}{}={}",
                    st.url,
                    sep,
                    pct_encode(&st.auth_key_name),
                    pct_encode(&st.auth_key_value)
                )
            } else {
                st.url.clone()
            }
        } else {
            let mut pairs: Vec<String> = active
                .iter()
                .map(|p| format!("{}={}", pct_encode(&p.key), pct_encode(&p.value)))
                .collect();
            if st.auth_type == "api-key"
                && st.auth_key_in == "query"
                && !st.auth_key_name.is_empty()
            {
                pairs.push(format!(
                    "{}={}",
                    pct_encode(&st.auth_key_name),
                    pct_encode(&st.auth_key_value)
                ));
            }
            let sep = if st.url.contains('?') { '&' } else { '?' };
            format!("{}{}{}", st.url, sep, pairs.join("&"))
        }
    };

    // Headers
    let mut headers: Vec<(String, String)> = st
        .req_headers
        .iter()
        .filter(|h| !h.key.is_empty())
        .map(|h| (h.key.clone(), h.value.clone()))
        .collect();

    // Auth → headers
    match st.auth_type.as_str() {
        "bearer" if !st.auth_token.is_empty() => {
            headers.push(("Authorization".into(), format!("Bearer {}", st.auth_token)));
        }
        "basic" => {
            let cred = base64::engine::general_purpose::STANDARD
                .encode(format!("{}:{}", st.auth_username, st.auth_password));
            headers.push(("Authorization".into(), format!("Basic {cred}")));
        }
        "api-key" if st.auth_key_in == "header" && !st.auth_key_name.is_empty() => {
            headers.push((st.auth_key_name.clone(), st.auth_key_value.clone()));
        }
        _ => {}
    }

    // Body
    let body = if is_body_method(&st.method) && !st.body.is_empty() {
        Some(st.body.as_bytes().to_vec())
    } else {
        None
    };

    HttpRequest {
        url,
        method: st.method.clone(),
        headers,
        body,
    }
}
