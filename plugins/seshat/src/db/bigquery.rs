//! Google BigQuery client over the host `http-client` import (issue #108).
//!
//! Uses BigQuery REST API v2 (`/bigquery/v2/projects/{project}/...`) over
//! HTTPS (or plain HTTP for local emulators).
//!
//! Mapping to Seshat's [`DbAdapter`] hierarchy:
//!   * database → GCP Project ID (`Profile.database`)
//!   * schema   → BigQuery Dataset
//!   * table    → BigQuery Table / View
//!
//! Authentication:
//!   * `AuthMode::ApiKey` / `AuthMode::Password`: OAuth2 Bearer access token
//!     stored in `Profile.password` (e.g. `gcloud auth print-access-token`).
//!   * `AuthMode::None`: no `Authorization` header (useful for local emulators).

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde_json::{json, Value};

use crate::bindings::thoth::plugin::http_client::{self, HttpRequest};
use crate::db::{
    AuthMode, Column, ColumnInfo, ConnectionDefaults, DbAdapter, Profile, QueryResult, TableDetail,
    TableInfo,
};

/// BigQuery implementation of [`DbAdapter`].
pub struct BigQuery;

fn enc(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

fn project_id(p: &Profile) -> Result<&str, String> {
    let proj = p.database.trim();
    if proj.is_empty() {
        return Err("BigQuery requires a GCP Project ID in the Database field".to_string());
    }
    Ok(proj)
}

impl DbAdapter for BigQuery {
    fn connection_defaults(&self) -> ConnectionDefaults {
        ConnectionDefaults {
            port: 443,
            user: "",
            database: "",
            database_placeholder: "gcp-project-id",
        }
    }

    fn test_connection(&self, p: &Profile) -> Result<String, String> {
        let proj = project_id(p)?;
        let path = format!("/bigquery/v2/projects/{}/datasets?maxResults=1", enc(proj));
        let _ = request(p, "GET", &path, None)?;
        Ok(format!("BigQuery · project {proj}"))
    }

    fn list_databases(&self, p: &Profile) -> Result<Vec<String>, String> {
        let proj = project_id(p)?;
        let resp = request(p, "GET", "/bigquery/v2/projects?maxResults=50", None);
        if let Ok(v) = resp {
            let mut projects: Vec<String> = v
                .get("projects")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| item.get("id").and_then(Value::as_str).map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            if !projects.is_empty() {
                if !projects.iter().any(|x| x == proj) {
                    projects.insert(0, proj.to_string());
                }
                return Ok(projects);
            }
        }
        Ok(vec![proj.to_string()])
    }

    fn list_schemas(&self, p: &Profile) -> Result<Vec<String>, String> {
        let proj = project_id(p)?;
        let path = format!("/bigquery/v2/projects/{}/datasets?maxResults=500", enc(proj));
        let v = request(p, "GET", &path, None)?;
        let mut out: Vec<String> = v
            .get("datasets")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|d| {
                        d.get("datasetReference")
                            .and_then(|r| r.get("datasetId"))
                            .and_then(Value::as_str)
                            .map(String::from)
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        Ok(out)
    }

    fn list_tables(&self, p: &Profile, schema: &str) -> Result<Vec<TableInfo>, String> {
        let proj = project_id(p)?;
        let path = format!(
            "/bigquery/v2/projects/{}/datasets/{}/tables?maxResults=500",
            enc(proj),
            enc(schema)
        );
        let v = request(p, "GET", &path, None)?;
        let mut out: Vec<TableInfo> = v
            .get("tables")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| {
                        let name = t
                            .get("tableReference")
                            .and_then(|r| r.get("tableId"))
                            .and_then(Value::as_str)?;
                        let kind = t
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("TABLE")
                            .to_string();
                        Some(TableInfo {
                            database: None,
                            schema: schema.to_string(),
                            name: name.to_string(),
                            kind,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    fn find_tables(&self, p: &Profile, query: &str) -> Result<Vec<TableInfo>, String> {
        let needle = query.trim().to_lowercase();
        let datasets = self.list_schemas(p)?;
        let mut matches = Vec::new();
        for ds in datasets.into_iter().take(25) {
            if let Ok(tables) = self.list_tables(p, &ds) {
                for t in tables {
                    if needle.is_empty()
                        || t.name.to_lowercase().contains(&needle)
                        || t.schema.to_lowercase().contains(&needle)
                    {
                        matches.push(t);
                    }
                }
            }
        }
        Ok(matches)
    }

    fn list_columns(
        &self,
        p: &Profile,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>, String> {
        Ok(self.describe_table(p, schema, table)?.columns)
    }

    fn describe_table(
        &self,
        p: &Profile,
        schema: &str,
        table: &str,
    ) -> Result<TableDetail, String> {
        let proj = project_id(p)?;
        let path = format!(
            "/bigquery/v2/projects/{}/datasets/{}/tables/{}",
            enc(proj),
            enc(schema),
            enc(table)
        );
        let v = request(p, "GET", &path, None)?;
        let fields = v
            .get("schema")
            .and_then(|s| s.get("fields"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let columns = fields
            .iter()
            .filter_map(|f| {
                let name = f.get("name").and_then(Value::as_str)?.to_string();
                let data_type = f
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("STRING")
                    .to_string();
                let mode = f
                    .get("mode")
                    .and_then(Value::as_str)
                    .unwrap_or("NULLABLE");
                let nullable = mode != "REQUIRED";
                let default = f
                    .get("defaultValueExpression")
                    .and_then(Value::as_str)
                    .map(String::from);
                Some(ColumnInfo {
                    name,
                    data_type,
                    nullable,
                    default,
                    primary_key: false,
                    unique: false,
                    foreign_key: None,
                })
            })
            .collect();

        let row_estimate = v
            .get("numRows")
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let size = v
            .get("numBytes")
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<i64>().ok())
            .map(crate::db::human_size)
            .unwrap_or_default();

        Ok(TableDetail {
            columns,
            indexes: Vec::new(),
            row_estimate,
            size,
        })
    }

    fn run_query(&self, p: &Profile, sql: &str) -> Result<QueryResult, String> {
        let proj = project_id(p)?;
        let path = format!("/bigquery/v2/projects/{}/queries", enc(proj));
        let mut body = json!({
            "query": sql,
            "useLegacySql": false,
            "timeoutMs": 30000
        });
        let ds = p.user.trim();
        if !ds.is_empty() {
            body["defaultDataset"] = json!({
                "projectId": proj,
                "datasetId": ds
            });
        }

        let mut resp = request(p, "POST", &path, Some(body))?;

        // Poll jobs.getQueryResults if the query didn't complete within timeoutMs.
        for _ in 0..30 {
            if resp.get("jobComplete").and_then(Value::as_bool).unwrap_or(true) {
                break;
            }
            let Some(job_id) = resp
                .get("jobReference")
                .and_then(|r| r.get("jobId"))
                .and_then(Value::as_str)
            else {
                break;
            };
            let poll_path = format!(
                "/bigquery/v2/projects/{}/queries/{}?timeoutMs=5000",
                enc(proj),
                enc(job_id)
            );
            resp = request(p, "GET", &poll_path, None)?;
        }

        parse_query_response(&resp)
    }
}

/// Parse a BigQuery `jobs.query` / `jobs.getQueryResults` JSON response into [`QueryResult`].
pub fn parse_query_response(v: &Value) -> Result<QueryResult, String> {
    if let Some(errs) = v.get("errors").and_then(Value::as_array) {
        if let Some(first) = errs.first() {
            let msg = first
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("BigQuery query error");
            return Err(msg.to_string());
        }
    }

    let fields = v
        .get("schema")
        .and_then(|s| s.get("fields"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let columns: Vec<Column> = fields
        .iter()
        .map(|f| Column {
            name: f
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            type_name: f
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("STRING")
                .to_string(),
        })
        .collect();

    let raw_rows = v
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut rows = Vec::with_capacity(raw_rows.len());
    for row in &raw_rows {
        let cells = row
            .get("f")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut parsed = Vec::with_capacity(columns.len());
        for (idx, cell) in cells.iter().enumerate() {
            let val = cell.get("v").cloned().unwrap_or(Value::Null);
            let ty = columns.get(idx).map(|c| c.type_name.as_str()).unwrap_or("");
            parsed.push(coerce_bq_value(val, ty));
        }
        rows.push(parsed);
    }

    let tag = v
        .get("numDmlAffectedRows")
        .and_then(Value::as_str)
        .map(|s| format!("DML {s}"));

    Ok(QueryResult {
        columns,
        rows,
        tag,
    })
}

fn coerce_bq_value(val: Value, ty: &str) -> Value {
    match val {
        Value::Null => Value::Null,
        Value::String(ref s) => match ty {
            "INTEGER" | "INT64" => s
                .parse::<i64>()
                .map(|n| Value::Number(n.into()))
                .unwrap_or(val),
            "FLOAT" | "FLOAT64" | "NUMERIC" | "BIGNUMERIC" => s
                .parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number)
                .unwrap_or(val),
            "BOOLEAN" | "BOOL" => match s.as_str() {
                "true" | "TRUE" => Value::Bool(true),
                "false" | "FALSE" => Value::Bool(false),
                _ => val,
            },
            _ => val,
        },
        other => other,
    }
}

fn request(p: &Profile, method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
    let scheme = if p.tls { "https" } else { "http" };
    let url = format!("{scheme}://{}:{}{}", p.host, p.port, path);

    let mut headers = Vec::new();
    match p.auth {
        AuthMode::None => {}
        AuthMode::Password | AuthMode::ApiKey => {
            let tok = p.password.trim();
            if !tok.is_empty() {
                headers.push(("Authorization".to_string(), format!("Bearer {tok}")));
            }
        }
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

    let resp = http_client::fetch(&req).map_err(|e| format!("BigQuery HTTP error: {}", e.message))?;
    let text = String::from_utf8_lossy(&resp.body).to_string();
    if !(200..300).contains(&resp.status) {
        let msg = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| {
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .unwrap_or(text);
        return Err(format!("BigQuery HTTP {} — {msg}", resp.status));
    }

    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).map_err(|e| format!("Invalid BigQuery JSON response: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bigquery_query_response_and_coerces_types() {
        let payload = json!({
            "jobComplete": true,
            "schema": {
                "fields": [
                    { "name": "id", "type": "INT64" },
                    { "name": "score", "type": "FLOAT64" },
                    { "name": "active", "type": "BOOL" },
                    { "name": "label", "type": "STRING" }
                ]
            },
            "rows": [
                { "f": [ { "v": "42" }, { "v": "3.5" }, { "v": "true" }, { "v": "alpha" } ] },
                { "f": [ { "v": null }, { "v": "0" }, { "v": "false" }, { "v": "beta" } ] }
            ]
        });

        let res = parse_query_response(&payload).unwrap();
        assert_eq!(res.columns.len(), 4);
        assert_eq!(res.columns[0].name, "id");
        assert_eq!(res.rows.len(), 2);
        assert_eq!(res.rows[0][0], json!(42));
        assert_eq!(res.rows[0][1], json!(3.5));
        assert_eq!(res.rows[0][2], json!(true));
        assert_eq!(res.rows[0][3], json!("alpha"));
        assert_eq!(res.rows[1][0], Value::Null);
        assert_eq!(res.rows[1][2], json!(false));
    }
}
