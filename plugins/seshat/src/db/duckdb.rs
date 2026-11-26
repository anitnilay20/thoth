//! DuckDB HTTP SQL bridge adapter over `http_client` (issue #105).
//!
//! Because DuckDB is an embedded C++ library and `seshat` compiles to
//! `wasm32-wasip1`, this adapter talks to a local or remote DuckDB HTTP SQL
//! server (e.g. `duckdb -ui`, `buenavista`, or a lightweight HTTP bridge
//! accepting `POST /` or `POST /query` with JSON/SQL).

use serde_json::{json, Value};

use crate::bindings::thoth::plugin::http_client::{self, HttpRequest};
use crate::db::{
    AuthMode, Column, ColumnInfo, ConnectionDefaults, DbAdapter, Profile, QueryResult, TableDetail,
    TableInfo,
};

/// DuckDB implementation of [`DbAdapter`].
pub struct DuckDb;

fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

impl DbAdapter for DuckDb {
    fn connection_defaults(&self) -> ConnectionDefaults {
        ConnectionDefaults {
            port: 8080,
            user: "",
            database: "memory",
            database_placeholder: "memory",
        }
    }

    fn test_connection(&self, p: &Profile) -> Result<String, String> {
        let res = self.run_query(p, "SELECT version() AS version")?;
        let ver = res
            .rows
            .first()
            .and_then(|r| r.first())
            .and_then(Value::as_str)
            .unwrap_or("connected");
        Ok(format!("DuckDB {ver}"))
    }

    fn list_databases(&self, p: &Profile) -> Result<Vec<String>, String> {
        let res = self.run_query(p, "SHOW DATABASES")?;
        let mut out: Vec<String> = res
            .rows
            .iter()
            .filter_map(|r| r.first().and_then(Value::as_str).map(String::from))
            .collect();
        if out.is_empty() {
            out.push("memory".to_string());
        }
        out.sort();
        Ok(out)
    }

    fn list_schemas(&self, p: &Profile) -> Result<Vec<String>, String> {
        let res = self.run_query(
            p,
            "SELECT DISTINCT schema_name FROM information_schema.schemata ORDER BY schema_name",
        )?;
        let mut out: Vec<String> = res
            .rows
            .iter()
            .filter_map(|r| r.first().and_then(Value::as_str).map(String::from))
            .collect();
        if out.is_empty() {
            out.push("main".to_string());
        }
        Ok(out)
    }

    fn list_tables(&self, p: &Profile, schema: &str) -> Result<Vec<TableInfo>, String> {
        let escaped = schema.replace('\'', "''");
        let sql = format!(
            "SELECT table_name, table_type FROM information_schema.tables \
             WHERE table_schema = '{escaped}' ORDER BY table_name"
        );
        let res = self.run_query(p, &sql)?;
        Ok(res
            .rows
            .into_iter()
            .filter_map(|r| {
                let name = r.first()?.as_str()?.to_string();
                let ty = r.get(1).and_then(Value::as_str).unwrap_or("BASE TABLE");
                let kind = if ty.contains("VIEW") { "view" } else { "table" };
                Some(TableInfo {
                    database: None,
                    schema: schema.to_string(),
                    name,
                    kind: kind.to_string(),
                })
            })
            .collect())
    }

    fn find_tables(&self, p: &Profile, query: &str) -> Result<Vec<TableInfo>, String> {
        let escaped = query.trim().replace('\'', "''");
        let sql = format!(
            "SELECT table_schema, table_name, table_type FROM information_schema.tables \
             WHERE table_name ILIKE '%{escaped}%' ORDER BY table_schema, table_name LIMIT 100"
        );
        let res = self.run_query(p, &sql)?;
        Ok(res
            .rows
            .into_iter()
            .filter_map(|r| {
                let schema = r.first()?.as_str()?.to_string();
                let name = r.get(1)?.as_str()?.to_string();
                let ty = r.get(2).and_then(Value::as_str).unwrap_or("BASE TABLE");
                let kind = if ty.contains("VIEW") { "view" } else { "table" };
                Some(TableInfo {
                    database: None,
                    schema,
                    name,
                    kind: kind.to_string(),
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
        schema: &str,
        table: &str,
    ) -> Result<TableDetail, String> {
        let fqn = if schema.is_empty() {
            quote_ident(table)
        } else {
            format!("{}.{}", quote_ident(schema), quote_ident(table))
        };
        let res = self.run_query(p, &format!("DESCRIBE {fqn}"))?;
        let columns = res
            .rows
            .iter()
            .filter_map(|r| {
                let name = r.first()?.as_str()?.to_string();
                let data_type = r
                    .get(1)
                    .and_then(Value::as_str)
                    .unwrap_or("VARCHAR")
                    .to_string();
                let nullable = r
                    .get(2)
                    .and_then(Value::as_str)
                    .map(|s| s.eq_ignore_ascii_case("YES"))
                    .unwrap_or(true);
                let primary_key = r
                    .get(3)
                    .and_then(Value::as_str)
                    .map(|s| s.contains("PRI"))
                    .unwrap_or(false);
                let default = r
                    .get(4)
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(String::from);
                Some(ColumnInfo {
                    name,
                    data_type,
                    nullable,
                    default,
                    primary_key,
                    unique: false,
                    foreign_key: None,
                })
            })
            .collect();

        Ok(TableDetail {
            columns,
            indexes: Vec::new(),
            row_estimate: 0,
            size: String::new(),
        })
    }

    fn run_query(&self, p: &Profile, sql: &str) -> Result<QueryResult, String> {
        let scheme = if p.tls { "https" } else { "http" };
        let url = format!("{scheme}://{}:{}/query", p.host, p.port);
        let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        if p.auth != AuthMode::None && !p.password.trim().is_empty() {
            headers.push((
                "Authorization".to_string(),
                format!("Bearer {}", p.password.trim()),
            ));
        }
        let body = json!({
            "sql": sql,
            "database": p.database,
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
            http_client::fetch(&req).map_err(|e| format!("DuckDB HTTP error: {}", e.message))?;
        let text = String::from_utf8_lossy(&resp.body).to_string();
        if !(200..300).contains(&resp.status) {
            return Err(format!("DuckDB HTTP {} — {text}", resp.status));
        }
        let v: Value = serde_json::from_str(&text)
            .map_err(|e| format!("Invalid DuckDB JSON response: {e}"))?;
        parse_duckdb_json(&v)
    }
}

/// Parse a DuckDB HTTP JSON response (`{ columns, rows }` or `{ meta, data }`).
pub fn parse_duckdb_json(v: &Value) -> Result<QueryResult, String> {
    if let Some(err) = v.get("error").and_then(Value::as_str) {
        return Err(err.to_string());
    }
    let meta = v
        .get("columns")
        .or_else(|| v.get("meta"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let columns: Vec<Column> = meta
        .iter()
        .map(|c| {
            if let Some(name) = c.as_str() {
                Column {
                    name: name.to_string(),
                    type_name: "any".to_string(),
                }
            } else {
                Column {
                    name: c
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    type_name: c
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("any")
                        .to_string(),
                }
            }
        })
        .collect();

    let raw_rows = v
        .get("rows")
        .or_else(|| v.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let rows = raw_rows
        .into_iter()
        .map(|r| r.as_array().cloned().unwrap_or_else(|| vec![r]))
        .collect();

    Ok(QueryResult {
        columns,
        rows,
        tag: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_duckdb_json_response() {
        let payload = json!({
            "columns": [
                { "name": "id", "type": "BIGINT" },
                { "name": "city", "type": "VARCHAR" }
            ],
            "rows": [
                [1, "Amsterdam"],
                [2, "Seattle"]
            ]
        });
        let res = parse_duckdb_json(&payload).unwrap();
        assert_eq!(res.columns.len(), 2);
        assert_eq!(res.columns[0].name, "id");
        assert_eq!(res.rows[1][1], json!("Seattle"));
    }
}
