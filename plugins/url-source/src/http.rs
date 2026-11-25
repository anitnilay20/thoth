use base64::Engine as _;

use crate::helper::{plugin_err, substitute_env};
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

/// Build an `HttpRequest` from the current plugin state, substituting `{{env}}` variables (#128).
pub fn build_request(st: &State) -> HttpRequest {
    let base_url = substitute_env(&st.url, &st.env_vars);
    // URL + query params
    let url = {
        let active: Vec<&KvPair> = st
            .params
            .iter()
            .filter(|p| p.enabled && !p.key.is_empty())
            .collect();
        if active.is_empty() {
            // Also handle api-key-in-query auth
            if st.auth_type == "api-key"
                && st.auth_key_in == "query"
                && !st.auth_key_name.is_empty()
            {
                let sep = if base_url.contains('?') { '&' } else { '?' };
                format!(
                    "{}{}{}={}",
                    base_url,
                    sep,
                    pct_encode(&substitute_env(&st.auth_key_name, &st.env_vars)),
                    pct_encode(&substitute_env(&st.auth_key_value, &st.env_vars))
                )
            } else {
                base_url.clone()
            }
        } else {
            let mut pairs: Vec<String> = active
                .iter()
                .map(|p| {
                    format!(
                        "{}={}",
                        pct_encode(&substitute_env(&p.key, &st.env_vars)),
                        pct_encode(&substitute_env(&p.value, &st.env_vars))
                    )
                })
                .collect();
            if st.auth_type == "api-key"
                && st.auth_key_in == "query"
                && !st.auth_key_name.is_empty()
            {
                pairs.push(format!(
                    "{}={}",
                    pct_encode(&substitute_env(&st.auth_key_name, &st.env_vars)),
                    pct_encode(&substitute_env(&st.auth_key_value, &st.env_vars))
                ));
            }
            let sep = if base_url.contains('?') { '&' } else { '?' };
            format!("{}{}{}", base_url, sep, pairs.join("&"))
        }
    };

    // Headers
    let mut headers: Vec<(String, String)> = st
        .req_headers
        .iter()
        .filter(|h| h.enabled && !h.key.is_empty())
        .map(|h| {
            (
                substitute_env(&h.key, &st.env_vars),
                substitute_env(&h.value, &st.env_vars),
            )
        })
        .collect();

    // Auth → headers
    match st.auth_type.as_str() {
        "bearer" if !st.auth_token.is_empty() => {
            headers.push((
                "Authorization".into(),
                format!("Bearer {}", substitute_env(&st.auth_token, &st.env_vars)),
            ));
        }
        "basic" => {
            let cred = base64::engine::general_purpose::STANDARD.encode(format!(
                "{}:{}",
                substitute_env(&st.auth_username, &st.env_vars),
                substitute_env(&st.auth_password, &st.env_vars)
            ));
            headers.push(("Authorization".into(), format!("Basic {cred}")));
        }
        "api-key" if st.auth_key_in == "header" && !st.auth_key_name.is_empty() => {
            headers.push((
                substitute_env(&st.auth_key_name, &st.env_vars),
                substitute_env(&st.auth_key_value, &st.env_vars),
            ));
        }
        _ => {}
    }

    // Body
    let body = if is_body_method(&st.method) && !st.body.is_empty() {
        Some(substitute_env(&st.body, &st.env_vars).into_bytes())
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
