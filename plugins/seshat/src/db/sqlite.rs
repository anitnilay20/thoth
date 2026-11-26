//! SQLite / libSQL Hrana HTTP v2 adapter over `http_client` (issue #106).
//!
//! Speaks the standard libSQL / `sqld` Hrana HTTP pipeline (`POST /v2/pipeline`)
//! so `seshat` (`wasm32-wasip1`) can browse schemas and run SQL against local
//! `sqld` instances or remote libSQL / Turso databases without linking C SQLite.

use serde_json::{json, Value};

use crate::bindings::thoth::plugin::http_client::{self, HttpRequest};
use crate::db::{
    AuthMode, Column, ColumnInfo, ConnectionDefaults, DbAdapter, IndexInfo, Profile, QueryResult,
    TableDetail, TableInfo,
};

/// SQLite / libSQL implementation of [`DbAdapter`].
pub struct Sqlite;

fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

impl DbAdapter for Sqlite {
    fn connection_defaults(&self) -> ConnectionDefaults {
        ConnectionDefaults {
            port: 8080,
            user: "",
            database: "main",
            database_placeholder: "main",
        }
    }

    fn test_connection(&self, p: &Profile) -> Result<String, String> {
        let res = self.run_query(p, "SELECT sqlite_version() AS version")?;
        let ver = res
            .rows
            .first()
            .and_then(|r| r.first())
            .and_then(Value::as_str)
            .unwrap_or("connected");
        Ok(format!("SQLite {ver}"))
    }

    fn list_databases(&self, p: &Profile) -> Result<Vec<String>, String> {
        let res = self.run_query(p, "PRAGMA database_list")?;
        let mut out: Vec<String> = res
            .rows
            .iter()
            .filter_map(|r| r.get(1).and_then(Value::as_str).map(String::from))
            .collect();
        if out.is_empty() {
            out.push("main".to_string());
        }
        Ok(out)
    }

    fn list_schemas(&self, p: &Profile) -> Result<Vec<String>, String> {
        self.list_databases(p)
    }

    fn list_tables(&self, p: &Profile, schema: &str) -> Result<Vec<TableInfo>, String> {
        let prefix = if schema.is_empty() || schema == "main" {
            String::new()
        } else {
            format!("{}.", quote_ident(schema))
        };
        let sql = format!(
            "SELECT name, type FROM {prefix}sqlite_master \
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' \
             ORDER BY name"
        );
        let res = self.run_query(p, &sql)?;
        Ok(res
            .rows
            .into_iter()
            .filter_map(|r| {
                let name = r.first()?.as_str()?.to_string();
                let kind = r
                    .get(1)
                    .and_then(Value::as_str)
                    .unwrap_or("table")
                    .to_string();
                Some(TableInfo {
                    database: None,
                    schema: if schema.is_empty() {
                        "main".to_string()
                    } else {
                        schema.to_string()
                    },
                    name,
                    kind,
                })
            })
            .collect())
    }

    fn find_tables(&self, p: &Profile, query: &str) -> Result<Vec<TableInfo>, String> {
        let escaped = query.trim().replace('\'', "''");
        let sql = format!(
            "SELECT name, type FROM sqlite_master \
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' \
             AND name LIKE '%{escaped}%' ORDER BY name LIMIT 100"
        );
        let res = self.run_query(p, &sql)?;
        Ok(res
            .rows
            .into_iter()
            .filter_map(|r| {
                let name = r.first()?.as_str()?.to_string();
                let kind = r
                    .get(1)
                    .and_then(Value::as_str)
                    .unwrap_or("table")
                    .to_string();
                Some(TableInfo {
                    database: None,
                    schema: "main".to_string(),
                    name,
                    kind,
                })
            })
            .collect())
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
        _schema: &str,
        table: &str,
    ) -> Result<TableDetail, String> {
        let safe_table = table.replace('\'', "''");
        let res = self.run_query(p, &format!("PRAGMA table_info('{safe_table}')"))?;
        let columns = res
            .rows
            .iter()
            .filter_map(|r| {
                let name = r.get(1)?.as_str()?.to_string();
                let data_type = r
                    .get(2)
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("TEXT")
                    .to_string();
                let notnull = r.get(3).and_then(Value::as_i64).unwrap_or(0) != 0;
                let default = r.get(4).and_then(Value::as_str).map(String::from);
                let primary_key = r.get(5).and_then(Value::as_i64).unwrap_or(0) != 0;
                Some(ColumnInfo {
                    name,
                    data_type,
                    nullable: !notnull,
                    default,
                    primary_key,
                    unique: false,
                    foreign_key: None,
                })
            })
            .collect();

        let mut indexes = Vec::new();
        if let Ok(idx_res) = self.run_query(p, &format!("PRAGMA index_list('{safe_table}')")) {
            for r in &idx_res.rows {
                if let Some(name) = r.get(1).and_then(Value::as_str) {
                    let unique = r.get(2).and_then(Value::as_i64).unwrap_or(0) != 0;
                    indexes.push(IndexInfo {
                        name: name.to_string(),
                        columns: Vec::new(),
                        unique,
                    });
                }
            }
        }

        Ok(TableDetail {
            columns,
            indexes,
            row_estimate: 0,
            size: String::new(),
        })
    }

    fn run_query(&self, p: &Profile, sql: &str) -> Result<QueryResult, String> {
        let scheme = if p.tls { "https" } else { "http" };
        let url = format!("{scheme}://{}:{}/v2/pipeline", p.host, p.port);
        let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        if p.auth != AuthMode::None && !p.password.trim().is_empty() {
            headers.push((
                "Authorization".to_string(),
                format!("Bearer {}", p.password.trim()),
            ));
        }
        let body = json!({
            "requests": [
                { "type": "execute", "stmt": { "sql": sql } },
                { "type": "close" }
            ]
        })
        .to_string()
        .into_bytes();

        let req = HttpRequest {
            url,
            method: "POST".to_string(),
            headers,
            body: Some(body),
        };
        let resp =
            http_client::fetch(&req).map_err(|e| format!("SQLite HTTP error: {}", e.message))?;
        let text = String::from_utf8_lossy(&resp.body).to_string();
        if !(200..300).contains(&resp.status) {
            return Err(format!("SQLite HTTP {} — {text}", resp.status));
        }
        let v: Value = serde_json::from_str(&text)
            .map_err(|e| format!("Invalid SQLite Hrana JSON response: {e}"))?;
        parse_hrana_pipeline(&v)
    }
}

/// Parse a libSQL Hrana `/v2/pipeline` JSON response into [`QueryResult`].
pub fn parse_hrana_pipeline(v: &Value) -> Result<QueryResult, String> {
    let first = v
        .get("results")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .ok_or_else(|| "Missing Hrana pipeline result".to_string())?;

    if first.get("type").and_then(Value::as_str) == Some("error") {
        let msg = first
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("SQLite query error");
        return Err(msg.to_string());
    }

    let result = first
        .get("response")
        .and_then(|r| r.get("result"))
        .unwrap_or(&Value::Null);

    let cols = result
        .get("cols")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let columns: Vec<Column> = cols
        .iter()
        .map(|c| Column {
            name: c
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            type_name: c
                .get("decltype")
                .and_then(Value::as_str)
                .unwrap_or("any")
                .to_string(),
        })
        .collect();

    let raw_rows = result
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut rows = Vec::with_capacity(raw_rows.len());
    for row in raw_rows {
        let cells = row.as_array().cloned().unwrap_or_default();
        let mut parsed = Vec::with_capacity(cells.len());
        for cell in cells {
            parsed.push(decode_hrana_value(&cell));
        }
        rows.push(parsed);
    }

    let affected = result.get("affected_row_count").and_then(Value::as_u64);
    let tag = affected.filter(|&n| n > 0).map(|n| format!("ROWS {n}"));

    Ok(QueryResult {
        columns,
        rows,
        tag,
    })
}

fn decode_hrana_value(cell: &Value) -> Value {
    let ty = cell.get("type").and_then(Value::as_str).unwrap_or("null");
    match ty {
        "null" => Value::Null,
        "integer" => cell
            .get("value")
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<i64>().ok())
            .map(|n| Value::Number(n.into()))
            .unwrap_or(Value::Null),
        "float" => cell
            .get("value")
            .and_then(Value::as_f64)
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        "text" | "blob" => cell.get("value").cloned().unwrap_or(Value::Null),
        _ => cell.get("value").cloned().unwrap_or(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hrana_pipeline_response() {
        let payload = json!({
            "results": [
                {
                    "type": "ok",
                    "response": {
                        "type": "execute",
                        "result": {
                            "cols": [
                                { "name": "id", "decltype": "INTEGER" },
                                { "name": "title", "decltype": "TEXT" }
                            ],
                            "rows": [
                                [ { "type": "integer", "value": "7" }, { "type": "text", "value": "Notes" } ],
                                [ { "type": "null" }, { "type": "text", "value": "Draft" } ]
                            ],
                            "affected_row_count": 0
                        }
                    }
                }
            ]
        });

        let res = parse_hrana_pipeline(&payload).unwrap();
        assert_eq!(res.columns.len(), 2);
        assert_eq!(res.rows[0][0], json!(7));
        assert_eq!(res.rows[0][1], json!("Notes"));
        assert_eq!(res.rows[1][0], Value::Null);
    }
}
