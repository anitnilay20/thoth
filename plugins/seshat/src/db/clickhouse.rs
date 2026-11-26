//! ClickHouse adapter over the HTTP interface (`FORMAT JSONCompact`) (#100).

use serde_json::Value;

use crate::bindings::thoth::plugin::http_client::{self, HttpRequest};
use crate::db::{
    Column, ColumnInfo, ConnectionDefaults, DbAdapter, Profile, QueryResult, TableDetail,
    TableInfo, human_size, int_at, str_at,
};

pub(crate) struct ClickHouse;

fn base_url(p: &Profile) -> String {
    let scheme = if p.tls { "https" } else { "http" };
    let db = if p.database.trim().is_empty() {
        "default"
    } else {
        p.database.trim()
    };
    format!("{scheme}://{}:{}/?database={db}", p.host.trim(), p.port)
}

fn needs_format_suffix(sql: &str) -> bool {
    let upper = sql.trim_start().to_ascii_uppercase();
    (upper.starts_with("SELECT")
        || upper.starts_with("SHOW")
        || upper.starts_with("DESCRIBE")
        || upper.starts_with("DESC ")
        || upper.starts_with("WITH")
        || upper.starts_with("EXPLAIN"))
        && !upper.contains("FORMAT ")
}

fn post_sql(p: &Profile, sql: &str) -> Result<String, String> {
    let body = if needs_format_suffix(sql) {
        format!("{sql}\nFORMAT JSONCompact")
    } else {
        sql.to_string()
    };

    let mut headers = vec![("Content-Type".to_string(), "text/plain; charset=utf-8".to_string())];
    if !p.user.is_empty() {
        headers.push(("X-ClickHouse-User".to_string(), p.user.clone()));
    }
    if !p.password.is_empty() {
        headers.push(("X-ClickHouse-Key".to_string(), p.password.clone()));
    }

    let resp = http_client::fetch(&HttpRequest {
        url: base_url(p),
        method: "POST".to_string(),
        headers,
        body: Some(body.into_bytes()),
    })
    .map_err(|e| format!("ClickHouse HTTP error: {}", e.message))?;

    let text = String::from_utf8_lossy(&resp.body).into_owned();
    if resp.status >= 400 {
        return Err(format!("ClickHouse HTTP {}: {}", resp.status, text.trim()));
    }
    Ok(text)
}

/// Parse ClickHouse `JSONCompact` payload into [`QueryResult`].
pub(crate) fn parse_json_compact(raw: &str) -> Result<QueryResult, String> {
    if raw.trim().is_empty() {
        return Ok(QueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            tag: Some("OK".to_string()),
        });
    }
    let val: Value =
        serde_json::from_str(raw).map_err(|e| format!("invalid ClickHouse JSONCompact: {e}"))?;
    let columns = val
        .get("meta")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|m| Column {
                    name: m
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    type_name: m
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("String")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    let rows = val
        .get("data")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|r| r.as_array().cloned())
                .collect()
        })
        .unwrap_or_default();
    Ok(QueryResult {
        columns,
        rows,
        tag: None,
    })
}

fn quote_lit(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

impl DbAdapter for ClickHouse {
    fn test_connection(&self, p: &Profile) -> Result<String, String> {
        let res = self.run_query(p, "SELECT version()")?;
        let ver = res
            .rows
            .first()
            .map(|r| str_at(r, 0))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "ClickHouse".to_string());
        Ok(format!("ClickHouse {ver}"))
    }

    fn connection_defaults(&self) -> ConnectionDefaults {
        ConnectionDefaults {
            port: 8123,
            user: "default",
            database: "default",
            database_placeholder: "default",
        }
    }

    fn list_databases(&self, p: &Profile) -> Result<Vec<String>, String> {
        let res = self.run_query(p, "SELECT name FROM system.databases ORDER BY name")?;
        Ok(res.rows.iter().map(|r| str_at(r, 0)).collect())
    }

    fn list_schemas(&self, p: &Profile) -> Result<Vec<String>, String> {
        let db = if p.database.trim().is_empty() {
            "default".to_string()
        } else {
            p.database.trim().to_string()
        };
        Ok(vec![db])
    }

    fn list_tables(&self, p: &Profile, schema: &str) -> Result<Vec<TableInfo>, String> {
        let sql = format!(
            "SELECT database, name, engine FROM system.tables WHERE database = {} ORDER BY name",
            quote_lit(schema)
        );
        let res = self.run_query(p, &sql)?;
        Ok(res
            .rows
            .iter()
            .map(|r| {
                let engine = str_at(r, 2);
                TableInfo {
                    database: Some(str_at(r, 0)),
                    schema: str_at(r, 0),
                    name: str_at(r, 1),
                    kind: if engine.eq_ignore_ascii_case("View")
                        || engine.eq_ignore_ascii_case("MaterializedView")
                    {
                        "view".to_string()
                    } else {
                        "table".to_string()
                    },
                }
            })
            .collect())
    }

    fn find_tables(&self, p: &Profile, query: &str) -> Result<Vec<TableInfo>, String> {
        let db = if p.database.trim().is_empty() {
            "default"
        } else {
            p.database.trim()
        };
        let sql = format!(
            "SELECT database, name, engine FROM system.tables WHERE database = {} AND positionCaseInsensitive(name, {}) > 0 ORDER BY name LIMIT 100",
            quote_lit(db),
            quote_lit(query)
        );
        let res = self.run_query(p, &sql)?;
        Ok(res
            .rows
            .iter()
            .map(|r| TableInfo {
                database: Some(str_at(r, 0)),
                schema: str_at(r, 0),
                name: str_at(r, 1),
                kind: "table".to_string(),
            })
            .collect())
    }

    fn list_columns(
        &self,
        p: &Profile,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>, String> {
        let sql = format!(
            "SELECT name, type, is_in_primary_key, default_expression FROM system.columns WHERE database = {} AND table = {} ORDER BY position",
            quote_lit(schema),
            quote_lit(table)
        );
        let res = self.run_query(p, &sql)?;
        Ok(res
            .rows
            .iter()
            .map(|r| {
                let data_type = str_at(r, 1);
                let def = str_at(r, 3);
                ColumnInfo {
                    name: str_at(r, 0),
                    nullable: data_type.starts_with("Nullable("),
                    data_type,
                    default: (!def.is_empty()).then_some(def),
                    primary_key: int_at(r, 2) != 0,
                    unique: false,
                    foreign_key: None,
                }
            })
            .collect())
    }

    fn describe_table(
        &self,
        p: &Profile,
        schema: &str,
        table: &str,
    ) -> Result<TableDetail, String> {
        let columns = self.list_columns(p, schema, table)?;
        let stats_sql = format!(
            "SELECT total_rows, total_bytes FROM system.tables WHERE database = {} AND table = {} LIMIT 1",
            quote_lit(schema),
            quote_lit(table)
        );
        let (row_estimate, size) = match self.run_query(p, &stats_sql) {
            Ok(res) if !res.rows.is_empty() => (
                int_at(&res.rows[0], 0),
                human_size(int_at(&res.rows[0], 1)),
            ),
            _ => (0, String::new()),
        };
        Ok(TableDetail {
            columns,
            indexes: Vec::new(),
            row_estimate,
            size,
        })
    }

    fn run_query(&self, p: &Profile, sql: &str) -> Result<QueryResult, String> {
        let raw = post_sql(p, sql)?;
        parse_json_compact(&raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_compact_response() {
        let raw = r#"{"meta":[{"name":"id","type":"UInt64"},{"name":"name","type":"String"}],"data":[[1,"ada"],[2,"linus"]]}"#;
        let res = parse_json_compact(raw).unwrap();
        assert_eq!(res.columns.len(), 2);
        assert_eq!(res.columns[0].name, "id");
        assert_eq!(res.rows.len(), 2);
        assert_eq!(res.rows[1][1], Value::String("linus".into()));
    }
}
