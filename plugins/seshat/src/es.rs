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
        Ok(to_column_infos(mapping_fields(p, table)?))
    }

    fn describe_table(
        &self,
        p: &Profile,
        _schema: &str,
        table: &str,
    ) -> Result<TableDetail, String> {
        let columns = to_column_infos(mapping_fields(p, table)?);

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
        match dialect(sql) {
            Dialect::Sql => run_sql(p, sql),
            Dialect::Esql => run_esql(p, sql),
            Dialect::QueryDsl => run_search(p, sql),
        }
    }
}

/// Run Elasticsearch SQL via `POST /_sql?format=json`. The response is already
/// columnar (`{columns:[{name,type}], rows:[[..]]}`) so it maps straight onto
/// [`QueryResult`].
fn run_sql(p: &Profile, query: &str) -> Result<QueryResult, String> {
    let body = serde_json::json!({ "query": query.trim() });
    let resp = request(p, "POST", "/_sql?format=json", Some(body))?;
    columnar_result(&resp, "rows", "SQL")
}

/// Run an ES|QL query via `POST /_query`. Same columnar shape as `_sql`, except
/// the data array is named `values`.
fn run_esql(p: &Profile, query: &str) -> Result<QueryResult, String> {
    let body = serde_json::json!({ "query": query.trim() });
    let resp = request(p, "POST", "/_query", Some(body))?;
    columnar_result(&resp, "values", "ES|QL")
}

/// Shared mapper for the two columnar endpoints. `rows_key` is `rows` for `_sql`
/// and `values` for ES|QL; `label` names the dialect in the result tag.
fn columnar_result(resp: &Value, rows_key: &str, label: &str) -> Result<QueryResult, String> {
    let columns: Vec<Column> = resp
        .get("columns")
        .and_then(Value::as_array)
        .map(|cols| {
            cols.iter()
                .map(|c| Column {
                    name: c
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    type_name: c
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    let rows: Vec<Vec<Value>> = resp
        .get(rows_key)
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|r| r.as_array().cloned().unwrap_or_default())
                .collect()
        })
        .unwrap_or_default();

    let tag = format!("{label} · {} row{}", rows.len(), plural(rows.len()));
    Ok(QueryResult {
        columns,
        rows,
        tag: Some(tag),
    })
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Run a Query-DSL search: `POST <index>/_search`, hits flattened into a grid.
fn run_search(p: &Profile, sql: &str) -> Result<QueryResult, String> {
    let (index, body) = split_query(sql);
    let path = format!("/{}/_search", enc(&index));
    let resp = request(p, "POST", &path, Some(body))?;

    let took = resp.get("took").and_then(Value::as_i64).unwrap_or(0);

    // Prefer server-side aggregations when present — a `size: 0` agg query
    // returns no hits, so the hits grid would otherwise be empty. Flatten the
    // buckets/metrics into a table instead.
    if let Some(aggs) = resp.get("aggregations").and_then(Value::as_object) {
        if !aggs.is_empty() {
            if let Some((names, rows)) = flatten_aggregations(aggs) {
                let columns = names
                    .iter()
                    .enumerate()
                    .map(|(i, name)| Column {
                        name: name.clone(),
                        type_name: infer_agg_type(&rows, i).to_string(),
                    })
                    .collect();
                let n = rows.len();
                return Ok(QueryResult {
                    columns,
                    rows,
                    tag: Some(format!("{n} group{} · took {took} ms", plural(n))),
                });
            }
        }
    }

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
        tag: Some(format!("{total} hits · took {took} ms")),
    })
}

/// One in-progress output row, keyed by column name (order tracked separately).
type AggRow = std::collections::HashMap<String, Value>;

/// Bucket scalar fields that are never sub-aggregations.
const RESERVED_BUCKET_KEYS: [&str; 7] = [
    "key",
    "key_as_string",
    "doc_count",
    "from",
    "to",
    "from_as_string",
    "to_as_string",
];

/// Flatten an Elasticsearch `aggregations` object into `(column order, rows)`.
///
/// Handles the common shapes: bucket aggs (`terms`, `histogram`,
/// `date_histogram`, `range`, `filters` — anything with a `buckets` array or
/// object), optionally nested, each contributing its key + `doc_count`, plus
/// metric sub-aggs (single-value `avg`/`sum`/`min`/`max`/`cardinality`/
/// `value_count`, and multi-value `stats`/`extended_stats`). Single-bucket aggs
/// (`filter`/`global`) annotate rows with a count and recurse. Returns `None`
/// when there's nothing tabular to render.
fn flatten_aggregations(
    aggs: &serde_json::Map<String, Value>,
) -> Option<(Vec<String>, Vec<Vec<Value>>)> {
    let mut columns: Vec<String> = Vec::new();
    let mut rows: Vec<AggRow> = vec![AggRow::new()];
    walk_aggs(aggs, &mut columns, &mut rows);
    if columns.is_empty() {
        return None;
    }
    let out = rows
        .into_iter()
        .map(|m| {
            columns
                .iter()
                .map(|c| m.get(c).cloned().unwrap_or(Value::Null))
                .collect()
        })
        .collect();
    Some((columns, out))
}

/// Walk one level of aggregations: metrics add columns to the current rows;
/// bucket aggs multiply rows (one per bucket) and recurse.
fn walk_aggs(
    aggs: &serde_json::Map<String, Value>,
    columns: &mut Vec<String>,
    rows: &mut Vec<AggRow>,
) {
    // 1) Metric aggs first — add columns without changing the row count.
    for (name, val) in aggs {
        let Some(obj) = val.as_object() else { continue };
        if obj.contains_key("buckets") || obj.contains_key("doc_count") {
            continue; // bucket / single-bucket → phase 2
        }
        add_metric(name, obj, columns, rows);
    }
    // 2) Bucket + single-bucket aggs — expand or annotate rows.
    for (name, val) in aggs {
        let Some(obj) = val.as_object() else { continue };
        if let Some(buckets) = obj.get("buckets") {
            let list = normalize_buckets(buckets);
            let key_col = name.clone();
            let count_col = format!("{name}.doc_count");
            ensure_col(columns, &key_col);
            ensure_col(columns, &count_col);
            let mut expanded: Vec<AggRow> = Vec::new();
            for base in rows.iter() {
                for (key, bucket) in &list {
                    let mut r = base.clone();
                    r.insert(key_col.clone(), key.clone());
                    r.insert(
                        count_col.clone(),
                        bucket.get("doc_count").cloned().unwrap_or(Value::Null),
                    );
                    let sub = sub_aggs(bucket);
                    let mut sub_rows = vec![r];
                    if !sub.is_empty() {
                        walk_aggs(&sub, columns, &mut sub_rows);
                    }
                    expanded.extend(sub_rows);
                }
            }
            *rows = expanded;
        } else if obj.contains_key("doc_count") {
            // Single-bucket agg (filter/global): annotate every row + recurse.
            let count_col = format!("{name}.doc_count");
            ensure_col(columns, &count_col);
            let c = obj.get("doc_count").cloned().unwrap_or(Value::Null);
            for r in rows.iter_mut() {
                r.insert(count_col.clone(), c.clone());
            }
            let sub = sub_aggs(obj);
            if !sub.is_empty() {
                walk_aggs(&sub, columns, rows);
            }
        }
    }
}

/// Add a metric aggregation's value(s) as column(s) on every current row.
fn add_metric(
    name: &str,
    obj: &serde_json::Map<String, Value>,
    columns: &mut Vec<String>,
    rows: &mut [AggRow],
) {
    if let Some(v) = obj.get("value") {
        // Single-value metric (avg/sum/min/max/cardinality/value_count).
        ensure_col(columns, name);
        for r in rows.iter_mut() {
            r.insert(name.to_string(), v.clone());
        }
        return;
    }
    // Multi-value metric (stats/extended_stats): flatten numeric leaves as
    // `name.<stat>` in encounter order.
    for (k, v) in obj {
        if v.is_number() {
            let col = format!("{name}.{k}");
            ensure_col(columns, &col);
            for r in rows.iter_mut() {
                r.insert(col.clone(), v.clone());
            }
        }
    }
}

/// Normalise a `buckets` value (array for terms/histogram/range, object for
/// filters / keyed ranges) into `(key, bucket-object)` pairs.
fn normalize_buckets(buckets: &Value) -> Vec<(Value, serde_json::Map<String, Value>)> {
    match buckets {
        Value::Array(arr) => arr
            .iter()
            .filter_map(|b| {
                b.as_object().map(|o| {
                    let key = o
                        .get("key_as_string")
                        .cloned()
                        .or_else(|| o.get("key").cloned())
                        .unwrap_or(Value::Null);
                    (key, o.clone())
                })
            })
            .collect(),
        Value::Object(map) => map
            .iter()
            .filter_map(|(name, b)| {
                b.as_object()
                    .map(|o| (Value::String(name.clone()), o.clone()))
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Object-valued fields of a bucket that are sub-aggregations (not scalars like
/// `key`/`doc_count`).
fn sub_aggs(obj: &serde_json::Map<String, Value>) -> serde_json::Map<String, Value> {
    obj.iter()
        .filter(|(k, v)| v.is_object() && !RESERVED_BUCKET_KEYS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Append `name` to `columns` if not already present.
fn ensure_col(columns: &mut Vec<String>, name: &str) {
    if !columns.iter().any(|c| c == name) {
        columns.push(name.to_string());
    }
}

/// Guess a type for an aggregation column from its values so numeric columns
/// right-align in the grid.
fn infer_agg_type(rows: &[Vec<Value>], col: usize) -> &'static str {
    let mut saw_number = false;
    for r in rows {
        match r.get(col) {
            None | Some(Value::Null) => {}
            Some(Value::Number(n)) => {
                saw_number = true;
                if n.as_i64().is_none() && n.as_u64().is_none() {
                    return "double";
                }
            }
            Some(Value::Bool(_)) => return "boolean",
            _ => return "keyword",
        }
    }
    if saw_number {
        "long"
    } else {
        "keyword"
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

    let resp = http_client::fetch(&req).map_err(|e| transport_error(p, &e.message))?;
    let text = String::from_utf8_lossy(&resp.body).to_string();

    if !(200..300).contains(&resp.status) {
        // Try to extract the structured ES error before falling back to raw text.
        let reason = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| {
                let err = v.get("error")?;
                // `error` is usually an object, but some endpoints return a string.
                if let Some(s) = err.as_str() {
                    return Some(s.to_string());
                }
                let ty = err.get("type").and_then(Value::as_str).unwrap_or("");
                let reason = err.get("reason").and_then(Value::as_str).unwrap_or("");
                Some(format!("{ty}: {reason}"))
            })
            .filter(|s| s.trim() != ":" && !s.trim().is_empty())
            .unwrap_or_else(|| text.clone());
        return Err(status_error(p, resp.status, &reason));
    }

    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|e| format!("invalid JSON response: {e}"))
}

/// Turn a transport-level failure (no HTTP response at all) into something
/// actionable. The host returns reqwest's message, which is accurate but terse —
/// "error sending request for url (http://localhost:9200/)" doesn't tell you the
/// cluster simply isn't running.
fn transport_error(p: &Profile, raw: &str) -> String {
    let target = format!("{}:{}", p.host, p.port);
    let lower = raw.to_lowercase();
    if lower.contains("connection refused") || lower.contains("error sending request") {
        return format!(
            "Couldn't reach {target} — is Elasticsearch running and listening on that port? ({raw})"
        );
    }
    if lower.contains("dns") || lower.contains("resolve") {
        return format!(
            "Couldn't resolve host '{}' — check the hostname. ({raw})",
            p.host
        );
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return format!(
            "Timed out connecting to {target} — the cluster may be starting up. ({raw})"
        );
    }
    // A TLS handshake failure against a plaintext cluster is a common mix-up.
    if lower.contains("tls") || lower.contains("ssl") || lower.contains("certificate") {
        return format!(
            "TLS error talking to {target} — if the cluster serves plain HTTP, turn off \"Require TLS\". ({raw})"
        );
    }
    format!("{raw} (target: {target})")
}

/// Add a hint to the common auth/permission statuses; other statuses pass the
/// server's own reason through unchanged.
fn status_error(p: &Profile, status: u16, reason: &str) -> String {
    let hint = match status {
        401 => match p.auth {
            AuthMode::None => Some(
                "this cluster requires authentication — set Auth to \"Username & password\" or \"API key\"",
            ),
            AuthMode::Password => Some("check the username and password"),
            AuthMode::ApiKey => Some("check the API key (it may be expired or from another cluster)"),
        },
        403 => Some("the credentials are valid but lack permission for this operation"),
        404 => Some("index not found — check the index name"),
        _ => None,
    };
    match hint {
        Some(h) => format!("HTTP {status} — {reason} ({h})"),
        None => format!("HTTP {status} — {reason}"),
    }
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

/// Build [`ColumnInfo`] rows from flattened mapping fields. Elasticsearch has no
/// nullability, primary keys, or foreign keys, so those carry fixed values —
/// shared by `list_columns` and `describe_table`.
fn to_column_infos(fields: Vec<(String, String)>) -> Vec<ColumnInfo> {
    fields
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
        .collect()
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

/// Which query language the editor text is written in. Elasticsearch exposes
/// three query surfaces and they need different endpoints and result shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dialect {
    /// Query DSL (JSON) — `POST <index>/_search`.
    QueryDsl,
    /// Elasticsearch SQL — `POST /_sql?format=json`.
    Sql,
    /// ES|QL — `POST /_query`.
    Esql,
}

/// Classify the editor text. Query DSL is the default; a leading SQL verb or an
/// ES|QL source command switches dialect. The check ignores a leading `<index>`
/// line only for Query DSL, since SQL/ES|QL name their source inside the query.
pub(crate) fn dialect(text: &str) -> Dialect {
    let t = text.trim_start();
    // A JSON body is unambiguously Query DSL.
    if t.starts_with('{') {
        return Dialect::QueryDsl;
    }
    let head = t
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    match head.as_str() {
        "SELECT" | "DESCRIBE" | "DESC" => Dialect::Sql,
        // `SHOW` exists in both dialects; ES|QL only has SHOW INFO, so treat the
        // bare verb as SQL (SHOW TABLES / SHOW COLUMNS are the common cases).
        "SHOW" => Dialect::Sql,
        "FROM" | "ROW" => Dialect::Esql,
        _ => Dialect::QueryDsl,
    }
}

/// Does this ES|QL query already contain a real `LIMIT` pipeline stage?
///
/// A plain substring search would also match identifiers — `rate_limit`,
/// `limits`, a column called `limit_reached` — and wrongly leave the query
/// uncapped. ES|QL stages are pipe-separated, so only a stage whose *first
/// token* is `LIMIT` counts.
fn has_esql_limit_stage(query: &str) -> bool {
    query.split('|').any(|stage| {
        stage
            .split_whitespace()
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case("LIMIT"))
    })
}

/// Elasticsearch's default `index.max_result_window`. A `size` beyond this is
/// rejected by the server, so the row cap is clamped here.
pub(crate) const MAX_RESULT_WINDOW: usize = 10_000;

/// Cap an Elasticsearch query to `n` rows, whichever dialect it's written in.
/// Returns `None` to run the query unchanged (already capped, uncappable, or
/// beyond the server's result window). This is the ES counterpart to
/// [`crate::sql::add_limit`] and preserves the same `+1 sentinel row` contract.
pub(crate) fn cap(text: &str, n: usize) -> Option<String> {
    match dialect(text) {
        // Elasticsearch SQL understands a normal LIMIT clause.
        Dialect::Sql => crate::sql::add_limit(text, n),
        // ES|QL caps with a pipeline stage: `FROM idx | LIMIT n`.
        Dialect::Esql => {
            let t = text.trim();
            if has_esql_limit_stage(t) {
                return None;
            }
            Some(format!("{t} | LIMIT {n}"))
        }
        Dialect::QueryDsl => add_size(text, n),
    }
}

/// Cap a Query-DSL search to `n` hits by injecting `"size": n` into its body.
///
/// `n` is clamped to [`MAX_RESULT_WINDOW`] — the server rejects a larger `size`,
/// and returning `None` here would leave the body unsized, which silently falls
/// back to Elasticsearch's default of 10 hits (the opposite of a large limit).
///
/// Returns `None` (leave the query alone) only when the query shouldn't be
/// capped at all:
///   * the body already sets `size` — respect the user's explicit choice, just
///     as `add_limit` bails when a `LIMIT` is already present;
///   * the body is an aggregation request, where `size` controls hits rather
///     than buckets and capping would mislead.
///
/// The returned text keeps the leading `<index>` line so it round-trips through
/// [`split_query`]; only the *submitted* copy is rewritten, never the editor's.
pub(crate) fn add_size(text: &str, n: usize) -> Option<String> {
    let n = n.min(MAX_RESULT_WINDOW);
    // SQL / ES|QL bodies aren't Query DSL — those are capped by their own
    // dialects (see `cap`), so never rewrite them here.
    if matches!(dialect(text), Dialect::Sql | Dialect::Esql) {
        return None;
    }
    let (index, mut body) = split_query(text);
    let obj = body.as_object_mut()?;
    if obj.contains_key("size") {
        return None;
    }
    if obj.contains_key("aggs") || obj.contains_key("aggregations") {
        return None;
    }
    obj.insert("size".to_string(), Value::from(n));
    Some(format!("{index}\n{body}"))
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
    fn flatten_terms_with_metric_subagg() {
        // terms(by_dept) → avg(avg_salary), mirroring a real `size:0` response.
        let aggs = serde_json::json!({
            "by_dept": {
                "buckets": [
                    { "key": "Engineering", "doc_count": 3, "avg_salary": { "value": 138333.0 } },
                    { "key": "Sales", "doc_count": 2, "avg_salary": { "value": 92000.0 } }
                ]
            }
        });
        let (cols, rows) = flatten_aggregations(aggs.as_object().unwrap()).unwrap();
        assert_eq!(cols, ["by_dept", "by_dept.doc_count", "avg_salary"]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], serde_json::json!("Engineering"));
        assert_eq!(rows[0][1], serde_json::json!(3));
        assert_eq!(rows[0][2], serde_json::json!(138333.0));
    }

    #[test]
    fn flatten_stats_metric() {
        let aggs = serde_json::json!({
            "price_stats": { "count": 3, "min": 1.0, "max": 9.0, "avg": 5.0, "sum": 15.0 }
        });
        let (cols, rows) = flatten_aggregations(aggs.as_object().unwrap()).unwrap();
        // Numeric leaves flattened as `price_stats.<stat>`.
        assert!(cols.contains(&"price_stats.avg".to_string()));
        assert!(cols.contains(&"price_stats.count".to_string()));
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn flatten_nested_buckets() {
        // terms(by_dept) → terms(by_active): rows multiply across levels.
        let aggs = serde_json::json!({
            "by_dept": { "buckets": [
                { "key": "Eng", "doc_count": 2, "by_active": { "buckets": [
                    { "key_as_string": "true", "key": 1, "doc_count": 2 }
                ]}}
            ]}
        });
        let (cols, rows) = flatten_aggregations(aggs.as_object().unwrap()).unwrap();
        assert_eq!(
            cols,
            [
                "by_dept",
                "by_dept.doc_count",
                "by_active",
                "by_active.doc_count"
            ]
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][2], serde_json::json!("true"));
    }

    #[test]
    fn flatten_none_when_empty() {
        assert!(flatten_aggregations(serde_json::json!({}).as_object().unwrap()).is_none());
    }

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

    // ── pagination (`size` injection) ───────────────────────────────────────

    #[test]
    fn add_size_injects_size_and_keeps_index() {
        let out = add_size("books\n{\"query\":{\"match_all\":{}}}", 101).unwrap();
        let (idx, body) = split_query(&out);
        assert_eq!(idx, "books");
        assert_eq!(body["size"], 101);
        // The original query is preserved alongside the injected cap.
        assert!(body["query"]["match_all"].is_object());
    }

    #[test]
    fn add_size_defaults_index_when_body_only() {
        let out = add_size("{\"query\":{\"match_all\":{}}}", 50).unwrap();
        let (idx, body) = split_query(&out);
        assert_eq!(idx, "_all");
        assert_eq!(body["size"], 50);
    }

    #[test]
    fn add_size_caps_a_bare_index_name() {
        // Clicking an index runs match_all; it must still be capped.
        let out = add_size("books", 101).unwrap();
        let (idx, body) = split_query(&out);
        assert_eq!(idx, "books");
        assert_eq!(body["size"], 101);
    }

    #[test]
    fn add_size_respects_user_supplied_size() {
        assert_eq!(add_size("books\n{\"size\":5}", 101), None);
    }

    #[test]
    fn add_size_skips_aggregation_queries() {
        assert_eq!(
            add_size(
                "books\n{\"aggs\":{\"by_year\":{\"terms\":{\"field\":\"year\"}}}}",
                101
            ),
            None
        );
    }

    #[test]
    fn add_size_clamps_to_max_result_window() {
        // Beyond the window we must still emit a `size` — returning the body
        // unsized would silently fall back to ES's default of 10 hits.
        let out = add_size("books", MAX_RESULT_WINDOW + 5000).unwrap();
        assert_eq!(split_query(&out).1["size"], MAX_RESULT_WINDOW);
        // At the boundary the requested value is used verbatim.
        let out = add_size("books", MAX_RESULT_WINDOW).unwrap();
        assert_eq!(split_query(&out).1["size"], MAX_RESULT_WINDOW);
    }

    #[test]
    fn add_size_leaves_sql_and_esql_alone() {
        assert_eq!(add_size("SELECT * FROM books", 101), None);
        assert_eq!(add_size("FROM books | LIMIT 5", 101), None);
    }

    // ── cap() dispatch per dialect ──────────────────────────────────────────

    #[test]
    fn cap_uses_size_for_query_dsl() {
        let out = cap("books", 101).unwrap();
        assert_eq!(split_query(&out).1["size"], 101);
    }

    #[test]
    fn cap_uses_limit_for_sql() {
        assert_eq!(
            cap("SELECT * FROM books", 101).as_deref(),
            Some("SELECT * FROM books LIMIT 101")
        );
        // An explicit LIMIT is respected.
        assert_eq!(cap("SELECT * FROM books LIMIT 5", 101), None);
    }

    #[test]
    fn cap_appends_pipeline_limit_for_esql() {
        assert_eq!(
            cap("FROM books", 101).as_deref(),
            Some("FROM books | LIMIT 101")
        );
        assert_eq!(cap("FROM books | LIMIT 5", 101), None);
    }

    #[test]
    fn esql_limit_stage_detection_ignores_identifiers() {
        // Real LIMIT stages (any casing, any spacing) are detected.
        assert!(has_esql_limit_stage("FROM books | LIMIT 5"));
        assert!(has_esql_limit_stage("FROM books | limit 5"));
        assert!(has_esql_limit_stage("FROM books |   LIMIT   5"));
        assert!(has_esql_limit_stage("FROM b | SORT x | LIMIT 10"));

        // Identifiers that merely contain "limit" must NOT count.
        assert!(!has_esql_limit_stage("FROM logs | WHERE rate_limit > 5"));
        assert!(!has_esql_limit_stage("FROM logs | KEEP limits"));
        assert!(!has_esql_limit_stage("FROM logs | KEEP limit_reached"));
        assert!(!has_esql_limit_stage("FROM books"));
    }

    #[test]
    fn cap_still_applies_when_limit_only_appears_as_identifier() {
        // Regression: a substring check would have skipped capping here.
        assert_eq!(
            cap("FROM logs | WHERE rate_limit > 5", 101).as_deref(),
            Some("FROM logs | WHERE rate_limit > 5 | LIMIT 101")
        );
    }

    // ── columnar (_sql / ES|QL) result mapping ──────────────────────────────

    #[test]
    fn columnar_result_maps_sql_rows() {
        // Shape captured from a live ES 8.15 `_sql?format=json` response.
        let resp = serde_json::json!({
            "columns": [{"name":"title","type":"text"},{"name":"year","type":"long"}],
            "rows": [["Thoth: Exploring Data", 2026], ["The Rust Programming Language", 2019]]
        });
        let qr = columnar_result(&resp, "rows", "SQL").unwrap();
        assert_eq!(qr.columns.len(), 2);
        assert_eq!(qr.columns[0].name, "title");
        assert_eq!(qr.columns[1].type_name, "long");
        assert_eq!(qr.rows.len(), 2);
        assert_eq!(qr.rows[0][1], 2026);
        assert_eq!(qr.tag.as_deref(), Some("SQL · 2 rows"));
    }

    #[test]
    fn columnar_result_maps_esql_values() {
        // ES|QL uses `values` rather than `rows`.
        let resp = serde_json::json!({
            "columns": [{"name":"title","type":"text"}],
            "values": [["Thoth: Exploring Data"]]
        });
        let qr = columnar_result(&resp, "values", "ES|QL").unwrap();
        assert_eq!(qr.rows.len(), 1);
        assert_eq!(qr.tag.as_deref(), Some("ES|QL · 1 row"));
    }

    #[test]
    fn columnar_result_tolerates_empty_response() {
        let qr = columnar_result(&serde_json::json!({}), "rows", "SQL").unwrap();
        assert!(qr.columns.is_empty());
        assert!(qr.rows.is_empty());
    }

    // ── dialect detection ───────────────────────────────────────────────────

    // ── dialect detection ───────────────────────────────────────────────────

    #[test]
    fn dialect_detects_each_surface() {
        assert_eq!(dialect("{\"query\":{}}"), Dialect::QueryDsl);
        assert_eq!(dialect("books"), Dialect::QueryDsl);
        assert_eq!(dialect("books\n{\"size\":1}"), Dialect::QueryDsl);
        assert_eq!(dialect("SELECT * FROM books"), Dialect::Sql);
        assert_eq!(dialect("  select author from books"), Dialect::Sql);
        assert_eq!(dialect("SHOW TABLES"), Dialect::Sql);
        assert_eq!(dialect("DESCRIBE books"), Dialect::Sql);
        assert_eq!(dialect("FROM books | LIMIT 5"), Dialect::Esql);
        assert_eq!(dialect("ROW a = 1"), Dialect::Esql);
    }

    // ── error messages ──────────────────────────────────────────────────────

    #[test]
    fn transport_error_explains_unreachable_cluster() {
        let p = Profile {
            host: "localhost".into(),
            port: 9200,
            ..Profile::default()
        };
        let msg = transport_error(&p, "error sending request for url (http://localhost:9200/)");
        assert!(msg.contains("Couldn't reach localhost:9200"), "got: {msg}");
        assert!(msg.contains("is Elasticsearch running"), "got: {msg}");
    }

    #[test]
    fn transport_error_flags_tls_mixup() {
        let p = Profile {
            host: "localhost".into(),
            port: 9200,
            ..Profile::default()
        };
        let msg = transport_error(&p, "invalid certificate: self signed");
        assert!(msg.contains("Require TLS"), "got: {msg}");
    }

    #[test]
    fn status_error_hints_per_auth_mode_on_401() {
        let none = Profile {
            auth: AuthMode::None,
            ..Profile::default()
        };
        assert!(status_error(&none, 401, "security_exception").contains("requires authentication"));

        let pw = Profile {
            auth: AuthMode::Password,
            ..Profile::default()
        };
        assert!(status_error(&pw, 401, "security_exception").contains("username and password"));

        let key = Profile {
            auth: AuthMode::ApiKey,
            ..Profile::default()
        };
        assert!(status_error(&key, 401, "security_exception").contains("API key"));
    }

    #[test]
    fn status_error_passes_through_unmapped_status() {
        let p = Profile::default();
        assert_eq!(status_error(&p, 500, "boom"), "HTTP 500 — boom");
    }
}
