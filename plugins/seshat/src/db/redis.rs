//! Redis adapter over RESP2 / `tcp-client` (#102).

use serde_json::Value;

use crate::bindings::thoth::plugin::tcp_client;
use crate::db::{
    Column, ColumnInfo, ConnectionDefaults, DbAdapter, Profile, QueryResult, TableDetail,
    TableInfo,
};

pub(crate) struct Redis;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RespValue {
    Simple(String),
    Error(String),
    Integer(i64),
    Bulk(Option<String>),
    Array(Option<Vec<RespValue>>),
}

impl RespValue {
    fn into_json(self) -> Value {
        match self {
            Self::Simple(s) => Value::String(s),
            Self::Error(e) => Value::String(format!("ERR: {e}")),
            Self::Integer(n) => Value::Number(n.into()),
            Self::Bulk(Some(s)) => Value::String(s),
            Self::Bulk(None) => Value::Null,
            Self::Array(Some(items)) => {
                Value::Array(items.into_iter().map(Self::into_json).collect())
            }
            Self::Array(None) => Value::Null,
        }
    }
}

/// Encode a command (`["GET", "foo"]`) as a RESP2 array.
pub(crate) fn encode_command(args: &[&str]) -> Vec<u8> {
    let mut out = format!("*{}\r\n", args.len()).into_bytes();
    for arg in args {
        out.extend_from_slice(format!("${}\r\n{}\r\n", arg.len(), arg).as_bytes());
    }
    out
}

/// Split a command string respecting single/double quotes.
pub(crate) fn split_command(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in input.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => cur.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !cur.is_empty() {
                    args.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        args.push(cur);
    }
    args
}

/// Parse one complete RESP2 value from `buf`, returning `(value, consumed_bytes)`.
pub(crate) fn parse_resp(buf: &[u8]) -> Result<Option<(RespValue, usize)>, String> {
    let Some(crlf) = buf.windows(2).position(|w| w == b"\r\n") else {
        return Ok(None);
    };
    let line = String::from_utf8_lossy(&buf[1..crlf]).into_owned();
    let mut pos = crlf + 2;
    match buf[0] {
        b'+' => Ok(Some((RespValue::Simple(line), pos))),
        b'-' => Ok(Some((RespValue::Error(line), pos))),
        b':' => {
            let n = line.trim().parse::<i64>().map_err(|e| e.to_string())?;
            Ok(Some((RespValue::Integer(n), pos)))
        }
        b'$' => {
            let len = line.trim().parse::<isize>().map_err(|e| e.to_string())?;
            if len < 0 {
                return Ok(Some((RespValue::Bulk(None), pos)));
            }
            let len = len as usize;
            if buf.len() < pos + len + 2 {
                return Ok(None);
            }
            let s = String::from_utf8_lossy(&buf[pos..pos + len]).into_owned();
            Ok(Some((RespValue::Bulk(Some(s)), pos + len + 2)))
        }
        b'*' => {
            let count = line.trim().parse::<isize>().map_err(|e| e.to_string())?;
            if count < 0 {
                return Ok(Some((RespValue::Array(None), pos)));
            }
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let Some((v, used)) = parse_resp(&buf[pos..])? else {
                    return Ok(None);
                };
                pos += used;
                items.push(v);
            }
            Ok(Some((RespValue::Array(Some(items)), pos)))
        }
        other => Err(format!("unexpected RESP prefix byte 0x{other:02x}")),
    }
}

fn send_recv(stream: u64, args: &[&str]) -> Result<RespValue, String> {
    let payload = encode_command(args);
    tcp_client::write(stream, &payload).map_err(|e| e.message)?;
    let mut buf = Vec::new();
    loop {
        let chunk = tcp_client::read(stream, 16 * 1024).map_err(|e| e.message)?;
        if chunk.is_empty() {
            return Err("Redis closed connection before completing reply".to_string());
        }
        buf.extend_from_slice(&chunk);
        if let Some((val, _)) = parse_resp(&buf)? {
            return match val {
                RespValue::Error(msg) => Err(msg),
                other => Ok(other),
            };
        }
    }
}

fn with_redis<T>(p: &Profile, f: impl FnOnce(u64) -> Result<T, String>) -> Result<T, String> {
    let stream = tcp_client::connect(p.host.trim(), p.port, p.tls)
        .map_err(|e| format!("Redis connect failed: {}", e.message))?;
    let res = (|| {
        if !p.password.is_empty() {
            if !p.user.is_empty() {
                send_recv(stream, &["AUTH", p.user.trim(), &p.password])?;
            } else {
                send_recv(stream, &["AUTH", &p.password])?;
            }
        }
        if !p.database.trim().is_empty() && p.database.trim() != "0" {
            send_recv(stream, &["SELECT", p.database.trim()])?;
        }
        f(stream)
    })();
    tcp_client::close(stream);
    res
}

impl DbAdapter for Redis {
    fn test_connection(&self, p: &Profile) -> Result<String, String> {
        with_redis(p, |s| {
            let reply = send_recv(s, &["PING"])?;
            Ok(match reply {
                RespValue::Simple(msg) => format!("Redis ({msg})"),
                _ => "Redis".to_string(),
            })
        })
    }

    fn connection_defaults(&self) -> ConnectionDefaults {
        ConnectionDefaults {
            port: 6379,
            user: "",
            database: "0",
            database_placeholder: "0",
        }
    }

    fn list_databases(&self, _p: &Profile) -> Result<Vec<String>, String> {
        Ok((0..16).map(|i| i.to_string()).collect())
    }

    fn list_schemas(&self, p: &Profile) -> Result<Vec<String>, String> {
        let db = if p.database.trim().is_empty() {
            "0".to_string()
        } else {
            p.database.trim().to_string()
        };
        Ok(vec![db])
    }

    fn list_tables(&self, p: &Profile, schema: &str) -> Result<Vec<TableInfo>, String> {
        with_redis(p, |s| {
            let reply = send_recv(s, &["SCAN", "0", "COUNT", "200"])?;
            let keys = match reply {
                RespValue::Array(Some(parts)) if parts.len() >= 2 => match &parts[1] {
                    RespValue::Array(Some(k)) => k
                        .iter()
                        .filter_map(|v| match v {
                            RespValue::Bulk(Some(name)) => Some(TableInfo {
                                database: Some(schema.to_string()),
                                schema: schema.to_string(),
                                name: name.clone(),
                                kind: "table".to_string(),
                            }),
                            _ => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            Ok(keys)
        })
    }

    fn find_tables(&self, p: &Profile, query: &str) -> Result<Vec<TableInfo>, String> {
        let pattern = if query.trim().is_empty() {
            "*".to_string()
        } else {
            format!("*{}*", query.trim())
        };
        let schema = if p.database.trim().is_empty() {
            "0"
        } else {
            p.database.trim()
        };
        with_redis(p, |s| {
            let reply = send_recv(s, &["SCAN", "0", "MATCH", &pattern, "COUNT", "100"])?;
            let keys = match reply {
                RespValue::Array(Some(parts)) if parts.len() >= 2 => match &parts[1] {
                    RespValue::Array(Some(k)) => k
                        .iter()
                        .filter_map(|v| match v {
                            RespValue::Bulk(Some(name)) => Some(TableInfo {
                                database: Some(schema.to_string()),
                                schema: schema.to_string(),
                                name: name.clone(),
                                kind: "table".to_string(),
                            }),
                            _ => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            Ok(keys)
        })
    }

    fn list_columns(
        &self,
        _p: &Profile,
        _schema: &str,
        _table: &str,
    ) -> Result<Vec<ColumnInfo>, String> {
        Ok(vec![
            ColumnInfo {
                name: "key".to_string(),
                data_type: "string".to_string(),
                ..Default::default()
            },
            ColumnInfo {
                name: "value".to_string(),
                data_type: "string".to_string(),
                ..Default::default()
            },
        ])
    }

    fn describe_table(
        &self,
        p: &Profile,
        _schema: &str,
        table: &str,
    ) -> Result<TableDetail, String> {
        with_redis(p, |s| {
            let ty = match send_recv(s, &["TYPE", table]) {
                Ok(RespValue::Simple(t)) => t,
                _ => "unknown".to_string(),
            };
            let ttl = match send_recv(s, &["TTL", table]) {
                Ok(RespValue::Integer(n)) => n.to_string(),
                _ => "-1".to_string(),
            };
            Ok(TableDetail {
                columns: vec![
                    ColumnInfo {
                        name: "type".to_string(),
                        data_type: ty,
                        ..Default::default()
                    },
                    ColumnInfo {
                        name: "ttl".to_string(),
                        data_type: ttl,
                        ..Default::default()
                    },
                ],
                indexes: Vec::new(),
                row_estimate: 1,
                size: String::new(),
            })
        })
    }

    fn run_query(&self, p: &Profile, sql: &str) -> Result<QueryResult, String> {
        let owned = split_command(sql);
        if owned.is_empty() {
            return Err("empty Redis command".to_string());
        }
        let refs: Vec<&str> = owned.iter().map(String::as_str).collect();
        let reply = with_redis(p, |s| send_recv(s, &refs))?;
        match reply {
            RespValue::Array(Some(items)) => Ok(QueryResult {
                columns: vec![Column {
                    name: "value".to_string(),
                    type_name: "text".to_string(),
                }],
                rows: items.into_iter().map(|v| vec![v.into_json()]).collect(),
                tag: None,
            }),
            other => Ok(QueryResult {
                columns: vec![Column {
                    name: "result".to_string(),
                    type_name: "text".to_string(),
                }],
                rows: vec![vec![other.into_json()]],
                tag: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_parses_resp2_values() {
        let cmd = encode_command(&["SET", "k", "hello world"]);
        assert_eq!(cmd, b"*3\r\n$3\r\nSET\r\n$1\r\nk\r\n$11\r\nhello world\r\n");

        let input = b"*2\r\n$3\r\nfoo\r\n:42\r\n";
        let (val, used) = parse_resp(input).unwrap().unwrap();
        assert_eq!(used, input.len());
        assert_eq!(
            val,
            RespValue::Array(Some(vec![
                RespValue::Bulk(Some("foo".into())),
                RespValue::Integer(42),
            ]))
        );
    }

    #[test]
    fn splits_quoted_redis_commands() {
        assert_eq!(
            split_command(r#"SET mykey "hello world""#),
            vec!["SET", "mykey", "hello world"]
        );
    }
}
