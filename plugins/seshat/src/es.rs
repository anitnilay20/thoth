//! Elasticsearch client over the host `http-client` import (issue #104).
//!
//! Unlike the Postgres/MySQL adapters — which speak a binary wire protocol over
//! the raw `tcp-client` shim — Elasticsearch is plain HTTP REST + JSON, so this
//! adapter drives the host `http-client::fetch` import. All calls are blocking
//! and run on the host db-runtime worker thread via the plugin's `query` export,
//! exactly like the SQL adapters.
//!
//! ES has no databases/schemas/tables, so its concepts are mapped onto Seshat's
//! shared Database → Schema → Table tree (mirroring how MySQL collapses the
//! schema layer):
//!   * database  → a single synthetic entry (the cluster)
//!   * schema    → a single synthetic "indices" namespace
//!   * table     → one Elasticsearch index (`_cat/indices`)
//!   * column    → one field from the index `_mapping`
//!
//! Auth: if `Profile.user` is empty and `Profile.password` is set, the password
//! is treated as an encoded **API key** (`Authorization: ApiKey <key>`);
//! otherwise **basic auth** (`Authorization: Basic base64(user:pass)`), matching
//! the two mechanisms Elasticsearch supports. `Profile.tls` selects http vs https.

use serde_json::{Map, Value};

use crate::bindings::thoth::plugin::http_client::{self, HttpRequest};
use crate::db::{
    AuthMode, Column, ColumnInfo, ConnectionDefaults, DbAdapter, Profile, QueryResult, TableDetail,
    TableInfo,
};

/// The synthetic schema name under which indices are listed in the tree.
const SCHEMA: &str = "indices";

/// The synthetic database name. ES has no databases, but the shared schema
/// browser is a Database → Schema → Table tree and only auto-loads the
/// connection's *default* database. So the connection defaults its database to
/// this same constant (see `connection_defaults`) and `list_databases` returns
/// it — that match is what makes indices load automatically after connecting.
const DATABASE: &str = "_all";

/// Elasticsearch implementation of [`DbAdapter`].
pub struct Elasticsearch;

impl DbAdapter for Elasticsearch {
    fn connection_defaults(&self) -> ConnectionDefaults {
        ConnectionDefaults {
            port: 9200,
            user: "elastic",
            // Must equal DATABASE so the schema tree auto-loads its indices.
            database: DATABASE,
            database_placeholder: DATABASE,
        }
    }

    fn test_connection(&self, p: &Profile) -> Result<String, String> {
        // `GET /` returns cluster name + version.
        let root = request(p, "GET", "/", None)?;
        let name = root
            .get("cluster_name")
            .and_then(Value::as_str)
            .or_else(|| root.get("name").and_then(Value::as_str))
            .unwrap_or("elasticsearch");
        let version = root
            .get("version")
            .and_then(|v| v.get("number"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        Ok(format!("{name} · Elasticsearch {version}"))
    }

    /// ES has no databases; expose a single synthetic entry whose name matches
    /// the connection default ([`DATABASE`]) so the schema browser auto-loads it.
    fn list_databases(&self, _p: &Profile) -> Result<Vec<String>, String> {
        Ok(vec![DATABASE.to_string()])
    }

    /// A single synthetic schema groups all indices.
    fn list_schemas(&self, _p: &Profile) -> Result<Vec<String>, String> {
        Ok(vec![SCHEMA.to_string()])
    }

    fn list_tables(&self, p: &Profile, schema: &str) -> Result<Vec<TableInfo>, String> {
        let indices = cat_indices(p)?;
        Ok(indices
            .into_iter()
            .map(|idx| TableInfo {
                database: None,
                schema: schema.to_string(),
                name: idx,
                kind: "table".to_string(),
            })
            .collect())
    }

    fn find_tables(&self, p: &Profile, query: &str) -> Result<Vec<TableInfo>, String> {
        let needle = query.to_lowercase();
        Ok(cat_indices(p)?
            .into_iter()
            .filter(|idx| idx.to_lowercase().contains(&needle))
            .take(200)
            .map(|idx| TableInfo {
                database: None,
                schema: SCHEMA.to_string(),
                name: idx,
                kind: "table".to_string(),
            })
            .collect())
    }

    fn list_columns(
        &self,
        p: &Profile,
        _schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>, String> {
        Ok(mapping_fields(p, table)?
            .into_iter()
            .map(|(name, data_type)| ColumnInfo {
                name,
                data_type,
                nullable: true,
                default: None,
                primary_key: false,
                unique: false,
                foreign_key: None,
            })
            .collect())
    }

    fn describe_table(
        &self,
        p: &Profile,
        _schema: &str,
        table: &str,
    ) -> Result<TableDetail, String> {
        let columns: Vec<ColumnInfo> = mapping_fields(p, table)?
            .into_iter()
            .map(|(name, data_type)| ColumnInfo {
                name,
                data_type,
                nullable: true,
                default: None,
                primary_key: false,
                unique: false,
                foreign_key: None,
            })
            .collect();

        // Doc count + store size come from `_cat/indices/<index>` (non-fatal).
        let (row_estimate, size) = index_stats(p, table).unwrap_or((0, String::new()));

        Ok(TableDetail {
            columns,
            indexes: Vec::new(),
            row_estimate,
            size,
        })
    }

    /// Run a Query-DSL search. The editor text is `[<index>]<newline>{json body}`:
    ///   * an optional first line names the target index (defaults to `_all`);
    ///   * the remainder is the `_search` request body (defaults to match_all).
    ///
    /// Hits are flattened — each `_source` object contributes columns (unioned
    /// across hits), plus `_id` and `_score`.
    fn run_query(&self, p: &Profile, sql: &str) -> Result<QueryResult, String> {
        let (index, body) = split_query(sql);
        let path = format!("/{}/_search", enc(&index));
        let resp = request(p, "POST", &path, Some(body))?;

        let took = resp.get("took").and_then(Value::as_i64).unwrap_or(0);
        let total = resp
            .get("hits")
            .and_then(|h| h.get("total"))
            .and_then(|t| {
                t.get("value")
                    .and_then(Value::as_i64)
                    .or_else(|| t.as_i64())
            })
            .unwrap_or(0);

        let hits = resp
            .get("hits")
            .and_then(|h| h.get("hits"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        // Column order: _id, _score, then source keys in first-seen order.
        let mut col_names: Vec<String> = vec!["_id".to_string(), "_score".to_string()];
        let mut seen: std::collections::HashSet<String> = col_names.iter().cloned().collect();
        for hit in &hits {
            if let Some(src) = hit.get("_source").and_then(Value::as_object) {
                for k in src.keys() {
                    if seen.insert(k.clone()) {
                        col_names.push(k.clone());
                    }
                }
            }
        }

        let rows: Vec<Vec<Value>> = hits
            .iter()
            .map(|hit| {
                col_names
                    .iter()
                    .map(|c| match c.as_str() {
                        "_id" => hit.get("_id").cloned().unwrap_or(Value::Null),
                        "_score" => hit.get("_score").cloned().unwrap_or(Value::Null),
                        other => hit
                            .get("_source")
                            .and_then(|s| s.get(other))
                            .cloned()
                            .unwrap_or(Value::Null),
                    })
                    .collect()
            })
            .collect();

        let columns = col_names
            .into_iter()
            .map(|name| Column {
                type_name: es_col_type(&name).to_string(),
                name,
            })
            .collect();

        Ok(QueryResult {
            columns,
            rows,
            tag: Some(format!("{} hits · took {took} ms", total)),
        })
    }
}

// ── HTTP plumbing ───────────────────────────────────────────────────────────

/// Perform one request against the cluster and parse the JSON response body.
/// Non-2xx responses are surfaced as `Err`, preferring the ES `error.reason`.
fn request(p: &Profile, method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
    let scheme = if p.tls { "https" } else { "http" };
    let url = format!("{scheme}://{}:{}{}", p.host, p.port, path);

    let mut headers: Vec<(String, String)> = Vec::new();
    if let Some(h) = auth_header(p) {
        headers.push(h);
    }
    let body_bytes = body.map(|v| {
        headers.push(("Content-Type".to_string(), "application/json".to_string()));
        v.to_string().into_bytes()
    });

    let req = HttpRequest {
        url,
        method: method.to_string(),
        headers,
        body: body_bytes,
    };

    let resp = http_client::fetch(&req).map_err(|e| e.message)?;
    let text = String::from_utf8_lossy(&resp.body).to_string();

    if !(200..300).contains(&resp.status) {
        // Try to extract the structured ES error before falling back to raw text.
        let reason = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| {
                let err = v.get("error")?;
                let ty = err.get("type").and_then(Value::as_str).unwrap_or("");
                let reason = err.get("reason").and_then(Value::as_str).unwrap_or("");
                Some(format!("{ty}: {reason}"))
            })
            .filter(|s| s.trim() != ":")
            .unwrap_or_else(|| text.clone());
        return Err(format!("HTTP {} — {reason}", resp.status));
    }

    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|e| format!("invalid JSON response: {e}"))
}

/// Build the `Authorization` header from the profile's auth mode. Returns `None`
/// for an open (no-auth) cluster.
fn auth_header(p: &Profile) -> Option<(String, String)> {
    match p.auth {
        AuthMode::None => None,
        AuthMode::ApiKey => Some((
            "Authorization".to_string(),
            format!("ApiKey {}", p.password),
        )),
        AuthMode::Password => {
            let token = base64_encode(format!("{}:{}", p.user, p.password).as_bytes());
            Some(("Authorization".to_string(), format!("Basic {token}")))
        }
    }
}

// ── ES REST helpers ─────────────────────────────────────────────────────────

/// List concrete (non-system) index names via `_cat/indices?format=json`.
fn cat_indices(p: &Profile) -> Result<Vec<String>, String> {
    let v = request(p, "GET", "/_cat/indices?format=json&h=index", None)?;
    let mut names: Vec<String> = v
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|row| row.get("index").and_then(Value::as_str))
                // Hide dot-prefixed system indices from the browser.
                .filter(|name| !name.starts_with('.'))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    Ok(names)
}

/// Flatten an index `_mapping` into `(field_path, es_type)` pairs.
fn mapping_fields(p: &Profile, index: &str) -> Result<Vec<(String, String)>, String> {
    let v = request(p, "GET", &format!("/{}/_mapping", enc(index)), None)?;
    // Response shape: { "<index>": { "mappings": { "properties": { ... } } } }.
    let properties = v
        .as_object()
        .and_then(|top| top.values().next()) // the (single) index entry
        .and_then(|idx| idx.get("mappings"))
        .and_then(|m| m.get("properties"))
        .and_then(Value::as_object);

    let mut out = Vec::new();
    if let Some(props) = properties {
        flatten_properties("", props, &mut out);
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Recursively walk mapping `properties`, emitting dotted field paths + types.
fn flatten_properties(prefix: &str, props: &Map<String, Value>, out: &mut Vec<(String, String)>) {
    for (name, spec) in props {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}.{name}")
        };
        match spec.get("type").and_then(Value::as_str) {
            Some(ty) => out.push((path, ty.to_string())),
            None => {
                // Object/nested field: recurse into its sub-properties.
                if let Some(sub) = spec.get("properties").and_then(Value::as_object) {
                    out.push((path.clone(), "object".to_string()));
                    flatten_properties(&path, sub, out);
                }
            }
        }
    }
}

/// Doc count + human store size from `_cat/indices/<index>` (best-effort).
fn index_stats(p: &Profile, index: &str) -> Result<(i64, String), String> {
    let path = format!(
        "/_cat/indices/{}?format=json&h=docs.count,store.size&bytes=b",
        enc(index)
    );
    let v = request(p, "GET", &path, None)?;
    let row = v.as_array().and_then(|a| a.first());
    let docs = row
        .and_then(|r| r.get("docs.count"))
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let bytes = row
        .and_then(|r| r.get("store.size"))
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    Ok((docs, human_size(bytes)))
}

// ── query text parsing ──────────────────────────────────────────────────────

/// Split editor text into `(index, body)`. An optional first line names the
/// target index; the rest is the JSON `_search` body. Both parts are optional:
///   * `""`                         → `_all`, match_all
///   * `books`                      → index `books`, match_all
///   * `{ "query": ... }`           → `_all`, given body
///   * `books\n{ "query": ... }`    → index `books`, given body
fn split_query(text: &str) -> (String, Value) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return ("_all".to_string(), match_all());
    }
    // If the whole thing is a JSON object, there's no index directive.
    if trimmed.starts_with('{') {
        let body = serde_json::from_str(trimmed).unwrap_or_else(|_| match_all());
        return ("_all".to_string(), body);
    }
    // Otherwise the first line is the index; the remainder (if any) is the body.
    let mut parts = trimmed.splitn(2, '\n');
    let index = parts.next().unwrap_or("").trim().to_string();
    let index = if index.is_empty() {
        "_all".to_string()
    } else {
        index
    };
    let rest = parts.next().unwrap_or("").trim();
    let body = if rest.is_empty() {
        match_all()
    } else {
        serde_json::from_str(rest).unwrap_or_else(|_| match_all())
    };
    (index, body)
}

fn match_all() -> Value {
    serde_json::json!({ "query": { "match_all": {} } })
}

/// Best-effort display type for a synthetic/result column.
fn es_col_type(name: &str) -> &'static str {
    match name {
        "_id" => "keyword",
        "_score" => "float",
        _ => "json",
    }
}

// ── small utilities ─────────────────────────────────────────────────────────

/// Minimal percent-encoding for an index name in a URL path segment. Index names
/// disallow most of these characters, but encoding keeps odd names safe.
fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'*' | b',' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Standard base64 (RFC 4648) — small enough to avoid a dependency.
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Format a byte count as a short human string (matches the SQL adapters' style).
fn human_size(bytes: i64) -> String {
    if bytes <= 0 {
        return String::new();
    }
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(
            base64_encode(b"elastic:changeme"),
            "ZWxhc3RpYzpjaGFuZ2VtZQ=="
        );
    }

    #[test]
    fn auth_header_none_when_open() {
        let p = Profile {
            auth: AuthMode::None,
            user: "ignored".to_string(),
            password: "ignored".to_string(),
            ..Profile::default()
        };
        assert_eq!(auth_header(&p), None);
    }

    #[test]
    fn auth_header_api_key_mode() {
        let p = Profile {
            auth: AuthMode::ApiKey,
            user: String::new(),
            password: "ABC123==".to_string(),
            ..Profile::default()
        };
        assert_eq!(auth_header(&p).unwrap().1, "ApiKey ABC123==");
    }

    #[test]
    fn auth_header_password_mode_uses_basic() {
        let p = Profile {
            auth: AuthMode::Password,
            user: "elastic".to_string(),
            password: "changeme".to_string(),
            ..Profile::default()
        };
        assert_eq!(auth_header(&p).unwrap().1, "Basic ZWxhc3RpYzpjaGFuZ2VtZQ==");
    }

    #[test]
    fn split_query_variants() {
        // empty → _all + match_all
        let (idx, body) = split_query("   ");
        assert_eq!(idx, "_all");
        assert_eq!(body, match_all());

        // bare index name
        let (idx, body) = split_query("books");
        assert_eq!(idx, "books");
        assert_eq!(body, match_all());

        // pure JSON body → _all
        let (idx, body) = split_query(r#"{"query":{"term":{"x":1}}}"#);
        assert_eq!(idx, "_all");
        assert_eq!(body["query"]["term"]["x"], 1);

        // index + body
        let (idx, body) = split_query("books\n{\"size\":5}");
        assert_eq!(idx, "books");
        assert_eq!(body["size"], 5);
    }

    #[test]
    fn split_query_parses_index_click_text() {
        // Must match `events::es_search_query`'s format: index on line 1, then a
        // match_all body. This is what clicking an index in the schema tree runs.
        let text = format!("{}\n{{ \"query\": {{ \"match_all\": {{}} }} }}", "books");
        let (idx, body) = split_query(&text);
        assert_eq!(idx, "books");
        assert_eq!(body, match_all());
    }

    #[test]
    fn flatten_nested_properties() {
        let props: Map<String, Value> = serde_json::from_value(serde_json::json!({
            "title": { "type": "text" },
            "location": { "properties": {
                "city": { "type": "keyword" },
                "geo": { "type": "geo_point" }
            }}
        }))
        .unwrap();
        let mut out = Vec::new();
        flatten_properties("", &props, &mut out);
        out.sort();
        assert!(out.contains(&("title".to_string(), "text".to_string())));
        assert!(out.contains(&("location".to_string(), "object".to_string())));
        assert!(out.contains(&("location.city".to_string(), "keyword".to_string())));
        assert!(out.contains(&("location.geo".to_string(), "geo_point".to_string())));
    }

    #[test]
    fn human_size_formats() {
        assert_eq!(human_size(0), "");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KB");
    }
}
