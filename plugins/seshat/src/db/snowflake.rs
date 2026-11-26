//! Snowflake SQL API v2 client over the host `http-client` import (issue #107).
//!
//! Uses Snowflake's REST SQL API (`/api/v2/statements`) over HTTPS:
//!   * Submit statement via `POST /api/v2/statements`
//!   * Poll async handle via `GET /api/v2/statements/{statementHandle}` on HTTP 202
//!
//! Authentication:
//!   * `Profile.password`: Bearer token — either a key-pair JWT
//!     (`X-Snowflake-Authorization-Token-Type: KEYPAIR_JWT`, auto-detected when
//!     the token has 3 dot-separated JWT segments), a Programmatic Access Token,
//!     or an OAuth2 token.
//!   * `Profile.database`: Database name, or `DATABASE.SCHEMA` (defaults to `PUBLIC`).
//!   * `Profile.user`: Optional Warehouse name.

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde_json::{json, Value};

use crate::bindings::thoth::plugin::http_client::{self, HttpRequest};
use crate::db::{
    AuthMode, Column, ColumnInfo, ConnectionDefaults, DbAdapter, Profile, QueryResult, TableDetail,
    TableInfo,
};

/// Snowflake SQL API v2 implementation of [`DbAdapter`].
pub struct Snowflake;

fn enc(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn db_and_schema(p: &Profile) -> (Option<&str>, Option<&str>) {
    let raw = p.database.trim();
    if raw.is_empty() {
        return (None, None);
    }
    if let Some((db, schema)) = raw.split_once('.') {
        (Some(db.trim()), Some(schema.trim()))
    } else {
        (Some(raw), Some("PUBLIC"))
    }
}

impl DbAdapter for Snowflake {
    fn connection_defaults(&self) -> ConnectionDefaults {
        ConnectionDefaults {
            port: 443,
            user: "",
            database: "",
            database_placeholder: "DATABASE.PUBLIC",
        }
    }

    fn test_connection(&self, p: &Profile) -> Result<String, String> {
        let res = self.run_query(p, "SELECT CURRENT_VERSION() AS version")?;
        let ver = res
            .rows
            .first()
            .and_then(|r| r.first())
            .and_then(Value::as_str)
            .unwrap_or("connected");
        Ok(format!("Snowflake {ver}"))
    }

    fn list_databases(&self, p: &Profile) -> Result<Vec<String>, String> {
        let res = self.run_query(p, "SHOW DATABASES")?;
        let name_col = res
            .columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case("name"))
            .unwrap_or(1);
        let mut out: Vec<String> = res
            .rows
            .iter()
            .filter_map(|r| r.get(name_col).and_then(Value::as_str).map(String::from))
            .collect();
        out.sort();
        Ok(out)
    }

    fn list_schemas(&self, p: &Profile) -> Result<Vec<String>, String> {
        let sql = match db_and_schema(p).0 {
            Some(db) => format!("SHOW SCHEMAS IN DATABASE {}", quote_ident(db)),
            None => "SHOW SCHEMAS".to_string(),
        };
        let res = self.run_query(p, &sql)?;
        let name_col = res
            .columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case("name"))
            .unwrap_or(1);
        let mut out: Vec<String> = res
            .rows
            .iter()
            .filter_map(|r| r.get(name_col).and_then(Value::as_str).map(String::from))
            .collect();
        out.sort();
        Ok(out)
    }

    fn list_tables(&self, p: &Profile, schema: &str) -> Result<Vec<TableInfo>, String> {
        let target = match db_and_schema(p).0 {
            Some(db) => format!("{}.{}", quote_ident(db), quote_ident(schema)),
            None => quote_ident(schema),
        };
        let mut out = Vec::new();
        for (stmt, kind) in [
            (format!("SHOW TABLES IN SCHEMA {target}"), "TABLE"),
            (format!("SHOW VIEWS IN SCHEMA {target}"), "VIEW"),
        ] {
            if let Ok(res) = self.run_query(p, &stmt) {
                let name_col = res
                    .columns
                    .iter()
                    .position(|c| c.name.eq_ignore_ascii_case("name"))
                    .unwrap_or(1);
                for r in &res.rows {
                    if let Some(name) = r.get(name_col).and_then(Value::as_str) {
                        out.push(TableInfo {
                            database: None,
                            schema: schema.to_string(),
                            name: name.to_string(),
                            kind: kind.to_string(),
                        });
                    }
                }
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    fn find_tables(&self, p: &Profile, query: &str) -> Result<Vec<TableInfo>, String> {
        let escaped = query.trim().replace('\'', "''");
        let prefix = db_and_schema(p)
            .0
            .map(|db| format!("{}.", quote_ident(db)))
            .unwrap_or_default();
        let sql = format!(
            "SELECT TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE, ROW_COUNT \
             FROM {prefix}INFORMATION_SCHEMA.TABLES \
             WHERE TABLE_NAME ILIKE '%{escaped}%' \
             ORDER BY TABLE_SCHEMA, TABLE_NAME LIMIT 100"
        );
        let res = self.run_query(p, &sql)?;
        Ok(res
            .rows
            .into_iter()
            .filter_map(|r| {
                let schema = r.first()?.as_str()?.to_string();
                let name = r.get(1)?.as_str()?.to_string();
                let kind = r
                    .get(2)
                    .and_then(Value::as_str)
                    .unwrap_or("TABLE")
                    .to_string();
                Some(TableInfo {
                    database: None,
                    schema,
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
        schema: &str,
        table: &str,
    ) -> Result<TableDetail, String> {
        let fqn = match db_and_schema(p).0 {
            Some(db) => format!(
                "{}.{}.{}",
                quote_ident(db),
                quote_ident(schema),
                quote_ident(table)
            ),
            None => format!("{}.{}", quote_ident(schema), quote_ident(table)),
        };
        let res = self.run_query(p, &format!("DESCRIBE TABLE {fqn}"))?;
        let name_col = res
            .columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case("name"))
            .unwrap_or(0);
        let type_col = res
            .columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case("type"))
            .unwrap_or(1);
        let null_col = res
            .columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case("null?"));
        let default_col = res
            .columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case("default"));
        let pk_col = res
            .columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case("primary key"));

        let columns = res
            .rows
            .iter()
            .filter_map(|r| {
                let name = r.get(name_col)?.as_str()?.to_string();
                let data_type = r
                    .get(type_col)
                    .and_then(Value::as_str)
                    .unwrap_or("VARCHAR")
                    .to_string();
                let nullable = null_col
                    .and_then(|i| r.get(i))
                    .and_then(Value::as_str)
                    .map(|s| s.eq_ignore_ascii_case("Y"))
                    .unwrap_or(true);
                let default = default_col
                    .and_then(|i| r.get(i))
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(String::from);
                let primary_key = pk_col
                    .and_then(|i| r.get(i))
                    .and_then(Value::as_str)
                    .map(|s| s.eq_ignore_ascii_case("Y"))
                    .unwrap_or(false);
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
        let mut body = json!({
            "statement": sql,
            "timeout": 60
        });
        let (db, schema) = db_and_schema(p);
        if let Some(db) = db {
            body["database"] = Value::String(db.to_string());
        }
        if let Some(schema) = schema {
            body["schema"] = Value::String(schema.to_string());
        }
        let wh = p.user.trim();
        if !wh.is_empty() {
            body["warehouse"] = Value::String(wh.to_string());
        }

        let (mut status, mut resp) = request(p, "POST", "/api/v2/statements", Some(body))?;

        // Snowflake returns HTTP 202 when execution is async; poll using statementHandle.
        for _ in 0..30 {
            if status != 202 {
                break;
            }
            let Some(handle) = resp.get("statementHandle").and_then(Value::as_str) else {
                break;
            };
            let poll_path = format!("/api/v2/statements/{}", enc(handle));
            let next = request(p, "GET", &poll_path, None)?;
            status = next.0;
            resp = next.1;
        }

        parse_statement_response(&resp)
    }
}

/// Parse a Snowflake SQL API v2 response into [`QueryResult`].
pub fn parse_statement_response(v: &Value) -> Result<QueryResult, String> {
    let row_types = v
        .get("resultSetMetaData")
        .and_then(|m| m.get("rowType"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let columns: Vec<Column> = row_types
        .iter()
        .map(|rt| Column {
            name: rt
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            type_name: rt
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("text")
                .to_string(),
        })
        .collect();

    let raw_rows = v
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut rows = Vec::with_capacity(raw_rows.len());
    for row in raw_rows {
        let cells = row.as_array().cloned().unwrap_or_default();
        let mut parsed = Vec::with_capacity(columns.len());
        for (idx, cell) in cells.into_iter().enumerate() {
            let ty = columns.get(idx).map(|c| c.type_name.as_str()).unwrap_or("");
            parsed.push(coerce_sf_value(cell, ty));
        }
        rows.push(parsed);
    }

    let tag = v
        .get("stats")
        .and_then(|s| s.get("numRowsInserted").or_else(|| s.get("numRowsUpdated")))
        .and_then(Value::as_u64)
        .map(|n| format!("ROWS {n}"));

    Ok(QueryResult {
        columns,
        rows,
        tag,
    })
}

fn coerce_sf_value(val: Value, ty: &str) -> Value {
    match val {
        Value::Null => Value::Null,
        Value::String(ref s) => match ty.to_lowercase().as_str() {
            "fixed" | "integer" | "bigint" | "smallint" | "tinyint" => {
                if let Ok(n) = s.parse::<i64>() {
                    Value::Number(n.into())
                } else if let Ok(f) = s.parse::<f64>() {
                    serde_json::Number::from_f64(f)
                        .map(Value::Number)
                        .unwrap_or(val)
                } else {
                    val
                }
            }
            "real" | "float" | "double" => s
                .parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number)
                .unwrap_or(val),
            "boolean" => match s.as_str() {
                "true" | "TRUE" => Value::Bool(true),
                "false" | "FALSE" => Value::Bool(false),
                _ => val,
            },
            _ => val,
        },
        other => other,
    }
}

fn request(
    p: &Profile,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<(u16, Value), String> {
    let scheme = if p.tls { "https" } else { "http" };
    let url = format!("{scheme}://{}:{}{}", p.host, p.port, path);

    let mut headers = vec![("Accept".to_string(), "application/json".to_string())];
    match p.auth {
        AuthMode::None => {}
        AuthMode::Password | AuthMode::ApiKey => {
            let tok = p.password.trim();
            if !tok.is_empty() {
                headers.push(("Authorization".to_string(), format!("Bearer {tok}")));
                let token_type = if tok.split('.').count() == 3 {
                    "KEYPAIR_JWT"
                } else {
                    "OAUTH"
                };
                headers.push((
                    "X-Snowflake-Authorization-Token-Type".to_string(),
                    token_type.to_string(),
                ));
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

    let resp =
        http_client::fetch(&req).map_err(|e| format!("Snowflake HTTP error: {}", e.message))?;
    let text = String::from_utf8_lossy(&resp.body).to_string();
    if !(200..300).contains(&resp.status) {
        let msg = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v.get("message").and_then(Value::as_str).map(String::from))
            .unwrap_or(text);
        return Err(format!("Snowflake HTTP {} — {msg}", resp.status));
    }

    let json = if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).map_err(|e| format!("Invalid Snowflake JSON response: {e}"))?
    };
    Ok((resp.status, json))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_snowflake_statement_response() {
        let payload = json!({
            "resultSetMetaData": {
                "rowType": [
                    { "name": "ID", "type": "fixed" },
                    { "name": "RATIO", "type": "real" },
                    { "name": "OK", "type": "boolean" },
                    { "name": "CITY", "type": "text" }
                ]
            },
            "data": [
                ["10", "0.75", "true", "Zurich"],
                [null, "1.25", "false", "Tokyo"]
            ]
        });

        let res = parse_statement_response(&payload).unwrap();
        assert_eq!(res.columns.len(), 4);
        assert_eq!(res.rows[0][0], json!(10));
        assert_eq!(res.rows[0][1], json!(0.75));
        assert_eq!(res.rows[0][2], json!(true));
        assert_eq!(res.rows[0][3], json!("Zurich"));
        assert_eq!(res.rows[1][0], Value::Null);
        assert_eq!(res.rows[1][2], json!(false));
    }
}
