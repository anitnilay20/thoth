//! Minimal Postgres client over the host `tcp-client` shim, using the
//! `postgres-protocol` codec (no async, no std::net). Simple-query protocol
//! only — enough for basic SELECTs and introspection.
//!
//! Auth supported: trust, cleartext, md5, SCRAM-SHA-256 (the stock `postgres`
//! default). Values arrive in text format and are decoded to native JSON for
//! the common scalar/json types, falling back to a string otherwise.

use std::io::{Read, Write};

use bytes::BytesMut;
use fallible_iterator::FallibleIterator;
use postgres_protocol::authentication;
use postgres_protocol::authentication::sasl::{ChannelBinding, ScramSha256};
use postgres_protocol::message::{backend, frontend};
use postgres_protocol::Oid;
use serde_json::Value;

use crate::db::{Column, ColumnInfo, DbAdapter, Profile, QueryResult, TableInfo};
use crate::shim::TcpShim;

const READ_CHUNK: usize = 16 * 1024;

/// Postgres implementation of [`DbAdapter`].
pub struct Postgres;

impl DbAdapter for Postgres {
    fn connection_defaults(&self) -> crate::db::ConnectionDefaults {
        crate::db::ConnectionDefaults {
            port: 5432,
            user: "postgres",
            database: "postgres",
            database_placeholder: "postgres",
        }
    }

    fn test_connection(&self, p: &Profile) -> Result<String, String> {
        let qr = run_query(p, "SELECT version()")?;
        Ok(qr
            .rows
            .first()
            .and_then(|r| r.first())
            .and_then(|v| v.as_str())
            .unwrap_or("connected")
            .to_string())
    }

    fn list_databases(&self, p: &Profile) -> Result<Vec<String>, String> {
        let qr = run_query(
            p,
            "SELECT datname FROM pg_database \
             WHERE datistemplate = false ORDER BY datname",
        )?;
        Ok(qr.rows.iter().map(|r| str_at(r, 0)).collect())
    }

    fn list_schemas(&self, p: &Profile) -> Result<Vec<String>, String> {
        let qr = run_query(
            p,
            "SELECT nspname FROM pg_namespace \
             WHERE nspname NOT LIKE 'pg\\_%' AND nspname <> 'information_schema' \
             ORDER BY nspname",
        )?;
        Ok(qr.rows.iter().map(|r| str_at(r, 0)).collect())
    }

    fn list_tables(&self, p: &Profile, schema: &str) -> Result<Vec<TableInfo>, String> {
        let sql = format!(
            "SELECT table_name, table_type FROM information_schema.tables \
             WHERE table_schema = {} ORDER BY table_name",
            quote_literal(schema)
        );
        let qr = run_query(p, &sql)?;
        Ok(qr
            .rows
            .iter()
            .map(|r| TableInfo {
                database: None,
                schema: schema.to_string(),
                name: str_at(r, 0),
                kind: if str_at(r, 1) == "VIEW" {
                    "view"
                } else {
                    "table"
                }
                .to_string(),
            })
            .collect())
    }

    fn find_tables(&self, p: &Profile, query: &str) -> Result<Vec<TableInfo>, String> {
        // A Postgres connection can only introspect the database it's connected
        // to, so search each database in turn (reconnecting) and merge results,
        // stopping once the overall cap is reached. Databases scanned is bounded
        // so a server with very many databases can't stall the search.
        const TOTAL_CAP: usize = 200;
        const MAX_DBS: usize = 30;
        let pattern = quote_literal(&format!("%{query}%"));
        let mut out: Vec<TableInfo> = Vec::new();

        for db in self.list_databases(p)?.into_iter().take(MAX_DBS) {
            if out.len() >= TOTAL_CAP {
                break;
            }
            let remaining = TOTAL_CAP - out.len();
            let sql = format!(
                "SELECT table_schema, table_name, table_type FROM information_schema.tables \
                 WHERE table_schema NOT IN ('pg_catalog', 'information_schema') \
                 AND table_name ILIKE {pattern} \
                 ORDER BY table_schema, table_name LIMIT {remaining}"
            );
            let dp = Profile {
                database: db.clone(),
                ..p.clone()
            };
            // A database we can't connect to (permissions) is skipped, not fatal.
            let Ok(qr) = run_query(&dp, &sql) else {
                continue;
            };
            for r in &qr.rows {
                out.push(TableInfo {
                    database: Some(db.clone()),
                    schema: str_at(r, 0),
                    name: str_at(r, 1),
                    kind: if str_at(r, 2) == "VIEW" {
                        "view"
                    } else {
                        "table"
                    }
                    .to_string(),
                });
            }
        }
        Ok(out)
    }

    fn list_columns(
        &self,
        p: &Profile,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>, String> {
        // Join key_column_usage to flag primary-key columns.
        let sql = format!(
            "SELECT c.column_name, c.data_type, c.is_nullable, c.column_default, \
                    (pk.column_name IS NOT NULL) AS is_pk \
             FROM information_schema.columns c \
             LEFT JOIN ( \
               SELECT kcu.column_name \
               FROM information_schema.table_constraints tc \
               JOIN information_schema.key_column_usage kcu \
                 ON kcu.constraint_name = tc.constraint_name \
                AND kcu.table_schema = tc.table_schema \
               WHERE tc.constraint_type = 'PRIMARY KEY' \
                 AND tc.table_schema = {schema} AND tc.table_name = {table} \
             ) pk ON pk.column_name = c.column_name \
             WHERE c.table_schema = {schema} AND c.table_name = {table} \
             ORDER BY c.ordinal_position",
            schema = quote_literal(schema),
            table = quote_literal(table)
        );
        let qr = run_query(p, &sql)?;
        Ok(qr
            .rows
            .iter()
            .map(|r| ColumnInfo {
                name: str_at(r, 0),
                data_type: str_at(r, 1),
                nullable: str_at(r, 2) == "YES",
                default: r.get(3).and_then(|v| v.as_str()).map(String::from),
                primary_key: bool_at(r, 4),
                unique: false,
                foreign_key: None,
            })
            .collect())
    }

    fn describe_table(
        &self,
        p: &Profile,
        schema: &str,
        table: &str,
    ) -> Result<crate::db::TableDetail, String> {
        let s = quote_literal(schema);
        let t = quote_literal(table);

        // Columns with PK / UNIQUE / FK flags, aggregated per column (a column may
        // participate in several constraints).
        let col_sql = format!(
            "SELECT c.column_name, c.data_type, c.is_nullable, c.column_default, \
                    COALESCE(bool_or(tc.constraint_type = 'PRIMARY KEY'), false) AS is_pk, \
                    COALESCE(bool_or(tc.constraint_type = 'UNIQUE'), false) AS is_unique, \
                    max(CASE WHEN tc.constraint_type = 'FOREIGN KEY' \
                             THEN ccu.table_name || '.' || ccu.column_name END) AS fk \
             FROM information_schema.columns c \
             LEFT JOIN information_schema.key_column_usage kcu \
               ON kcu.table_schema = c.table_schema AND kcu.table_name = c.table_name \
              AND kcu.column_name = c.column_name \
             LEFT JOIN information_schema.table_constraints tc \
               ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema \
             LEFT JOIN information_schema.constraint_column_usage ccu \
               ON ccu.constraint_name = tc.constraint_name AND tc.constraint_type = 'FOREIGN KEY' \
             WHERE c.table_schema = {s} AND c.table_name = {t} \
             GROUP BY c.column_name, c.data_type, c.is_nullable, c.column_default, c.ordinal_position \
             ORDER BY c.ordinal_position"
        );
        let columns = run_query(p, &col_sql)?
            .rows
            .iter()
            .map(|r| ColumnInfo {
                name: str_at(r, 0),
                data_type: str_at(r, 1),
                nullable: str_at(r, 2) == "YES",
                default: r.get(3).and_then(|v| v.as_str()).map(String::from),
                primary_key: bool_at(r, 4),
                unique: bool_at(r, 5) && !bool_at(r, 4),
                foreign_key: r.get(6).and_then(|v| v.as_str()).map(String::from),
            })
            .collect();

        // Indexes (non-fatal): name, unique flag, and ordered column list.
        let idx_sql = format!(
            "SELECT i.relname, ix.indisunique, \
                    array_to_string(array_agg(a.attname ORDER BY k.ord), ',') \
             FROM pg_index ix \
             JOIN pg_class i ON i.oid = ix.indexrelid \
             JOIN pg_class tb ON tb.oid = ix.indrelid \
             JOIN pg_namespace n ON n.oid = tb.relnamespace \
             JOIN unnest(ix.indkey) WITH ORDINALITY AS k(attnum, ord) ON true \
             JOIN pg_attribute a ON a.attrelid = tb.oid AND a.attnum = k.attnum \
             WHERE n.nspname = {s} AND tb.relname = {t} \
             GROUP BY i.relname, ix.indisunique ORDER BY i.relname"
        );
        let indexes = run_query(p, &idx_sql)
            .map(|qr| {
                qr.rows
                    .iter()
                    .map(|r| crate::db::IndexInfo {
                        name: str_at(r, 0),
                        unique: bool_at(r, 1),
                        columns: str_at(r, 2)
                            .split(',')
                            .filter(|c| !c.is_empty())
                            .map(String::from)
                            .collect(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Estimated rows + total size (non-fatal).
        let stat_sql = format!(
            "SELECT c.reltuples::bigint, \
                    pg_size_pretty(pg_total_relation_size(c.oid)) \
             FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = {s} AND c.relname = {t}"
        );
        let (row_estimate, size) = run_query(p, &stat_sql)
            .ok()
            .and_then(|qr| qr.rows.into_iter().next())
            .map(|r| (int_at(&r, 0).max(0), str_at(&r, 1)))
            .unwrap_or((0, String::new()));

        Ok(crate::db::TableDetail {
            columns,
            indexes,
            row_estimate,
            size,
        })
    }

    fn run_query(&self, p: &Profile, sql: &str) -> Result<QueryResult, String> {
        run_query(p, sql)
    }
}

/// Read an integer cell, tolerating either a JSON number or a numeric string.
fn int_at(row: &[Value], i: usize) -> i64 {
    row.get(i)
        .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())))
        .unwrap_or(0)
}

fn str_at(row: &[Value], i: usize) -> String {
    row.get(i)
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_default()
}

fn bool_at(row: &[Value], i: usize) -> bool {
    row.get(i).and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Quote a string as a Postgres SQL literal (for `information_schema` filters).
fn quote_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Connect, run `sql` via the simple-query protocol, return typed columns + rows.
/// Blocking — runs on the host query worker.
pub fn run_query(p: &Profile, sql: &str) -> Result<QueryResult, String> {
    // Postgres negotiates TLS *after* connecting (SSLRequest), not on connect —
    // so always open plaintext, then upgrade the stream in place when requested.
    let mut conn = TcpShim::connect(&p.host, p.port, false).map_err(|e| e.to_string())?;
    if p.tls {
        negotiate_ssl(&mut conn)?;
        conn.start_tls(&p.host).map_err(|e| e.to_string())?;
    }
    let mut out = BytesMut::new();
    let mut inbuf = BytesMut::new();
    let mut scratch = vec![0u8; READ_CHUNK];

    // ── Startup ──
    frontend::startup_message(
        [
            ("user", p.user.as_str()),
            ("database", p.database.as_str()),
            ("client_encoding", "UTF8"),
        ],
        &mut out,
    )
    .map_err(|e| e.to_string())?;
    flush(&mut conn, &mut out)?;

    // ── Auth handshake until ReadyForQuery ──
    authenticate(&mut conn, &mut out, &mut inbuf, &mut scratch, p)?;

    // ── Simple query ──
    frontend::query(sql, &mut out).map_err(|e| e.to_string())?;
    flush(&mut conn, &mut out)?;

    let mut columns: Vec<Column> = Vec::new();
    let mut oids: Vec<Oid> = Vec::new();
    let mut rows: Vec<Vec<Value>> = Vec::new();
    let mut tag: Option<String> = None;
    const MULTI_RESULT_ERR: &str =
        "multiple result sets are not supported; run one statement at a time";
    loop {
        match read_message(&mut conn, &mut inbuf, &mut scratch)? {
            backend::Message::RowDescription(body) => {
                // A prior statement already completed (tag set) — this is a
                // second result set. We return a single result, so reject it
                // rather than mixing rows from different schemas.
                if tag.is_some() {
                    return Err(MULTI_RESULT_ERR.to_string());
                }
                columns.clear();
                oids.clear();
                let mut fields = body.fields();
                while let Some(f) = fields.next().map_err(|e: std::io::Error| e.to_string())? {
                    let oid = f.type_oid();
                    columns.push(Column {
                        name: f.name().to_string(),
                        type_name: type_name(oid),
                    });
                    oids.push(oid);
                }
            }
            backend::Message::DataRow(body) => rows.push(row_to_values(&oids, &body)?),
            backend::Message::CommandComplete(body) => {
                // Same guard for back-to-back statements with no result rows
                // (e.g. two `UPDATE`s): a second command tag means a second
                // result set, which we don't support.
                if tag.is_some() {
                    return Err(MULTI_RESULT_ERR.to_string());
                }
                tag = body.tag().ok().map(|t| t.to_string());
            }
            backend::Message::ErrorResponse(body) => return Err(error_message(&body)),
            backend::Message::ReadyForQuery(_) => break,
            _ => {} // EmptyQueryResponse, NoticeResponse, ParameterStatus, …
        }
    }

    frontend::terminate(&mut out);
    let _ = flush(&mut conn, &mut out);
    Ok(QueryResult { columns, rows, tag })
}

/// Postgres SSL negotiation: send an SSLRequest and read the server's single
/// byte reply — `S` to proceed with TLS, `N` if the server won't do SSL.
fn negotiate_ssl(conn: &mut TcpShim) -> Result<(), String> {
    use bytes::BufMut;
    let mut req = BytesMut::with_capacity(8);
    req.put_i32(8); // total message length
    req.put_i32(80_877_103); // SSLRequest magic code
    conn.write_all(&req).map_err(|e| e.to_string())?;
    conn.flush().map_err(|e| e.to_string())?;
    let mut b = [0u8; 1];
    conn.read_exact(&mut b).map_err(|e| e.to_string())?;
    match b[0] {
        b'S' => Ok(()),
        b'N' => Err("server does not support SSL, but TLS was requested".to_string()),
        other => Err(format!("unexpected SSL negotiation reply: 0x{other:02x}")),
    }
}

fn authenticate(
    conn: &mut TcpShim,
    out: &mut BytesMut,
    inbuf: &mut BytesMut,
    scratch: &mut [u8],
    p: &Profile,
) -> Result<(), String> {
    loop {
        match read_message(conn, inbuf, scratch)? {
            backend::Message::AuthenticationOk => {} // proceed to ReadyForQuery
            backend::Message::AuthenticationCleartextPassword => {
                frontend::password_message(p.password.as_bytes(), out)
                    .map_err(|e| e.to_string())?;
                flush(conn, out)?;
            }
            backend::Message::AuthenticationMd5Password(body) => {
                let hashed =
                    authentication::md5_hash(p.user.as_bytes(), p.password.as_bytes(), body.salt());
                frontend::password_message(hashed.as_bytes(), out).map_err(|e| e.to_string())?;
                flush(conn, out)?;
            }
            backend::Message::AuthenticationSasl(body) => {
                scram_auth(conn, out, inbuf, scratch, p, body)?
            }
            backend::Message::ReadyForQuery(_) => return Ok(()),
            backend::Message::ErrorResponse(body) => return Err(error_message(&body)),
            _ => {} // ParameterStatus, BackendKeyData, NoticeResponse
        }
    }
}

fn scram_auth(
    conn: &mut TcpShim,
    out: &mut BytesMut,
    inbuf: &mut BytesMut,
    scratch: &mut [u8],
    p: &Profile,
    body: backend::AuthenticationSaslBody,
) -> Result<(), String> {
    let mechs: Vec<String> = body
        .mechanisms()
        .map(|m| Ok(m.to_string()))
        .collect()
        .map_err(|e: std::io::Error| e.to_string())?;
    if !mechs.iter().any(|m| m == "SCRAM-SHA-256") {
        return Err(format!("unsupported SASL mechanisms: {mechs:?}"));
    }

    let mut scram = ScramSha256::new(p.password.as_bytes(), ChannelBinding::unsupported());
    frontend::sasl_initial_response("SCRAM-SHA-256", scram.message(), out)
        .map_err(|e| e.to_string())?;
    flush(conn, out)?;

    match read_message(conn, inbuf, scratch)? {
        backend::Message::AuthenticationSaslContinue(c) => {
            scram.update(c.data()).map_err(|e| e.to_string())?;
        }
        backend::Message::ErrorResponse(b) => return Err(error_message(&b)),
        _ => return Err("unexpected message during SASL continue".into()),
    }

    frontend::sasl_response(scram.message(), out).map_err(|e| e.to_string())?;
    flush(conn, out)?;

    match read_message(conn, inbuf, scratch)? {
        backend::Message::AuthenticationSaslFinal(f) => {
            scram.finish(f.data()).map_err(|e| e.to_string())?;
        }
        backend::Message::ErrorResponse(b) => return Err(error_message(&b)),
        _ => return Err("unexpected message during SASL final".into()),
    }
    Ok(())
}

fn row_to_values(oids: &[Oid], body: &backend::DataRowBody) -> Result<Vec<Value>, String> {
    let ranges: Vec<Option<std::ops::Range<usize>>> = body
        .ranges()
        .collect()
        .map_err(|e: std::io::Error| e.to_string())?;
    let buf = body.buffer();
    let mut values = Vec::with_capacity(ranges.len());
    for (i, range) in ranges.into_iter().enumerate() {
        let value = match range {
            Some(r) => decode_value(oids.get(i).copied().unwrap_or(0), &buf[r]),
            None => Value::Null,
        };
        values.push(value);
    }
    Ok(values)
}

/// Decode a text-format cell to native JSON for the common types; string otherwise.
fn decode_value(oid: Oid, bytes: &[u8]) -> Value {
    let text = String::from_utf8_lossy(bytes);
    match oid {
        16 => match text.as_ref() {
            // bool
            "t" => Value::Bool(true),
            "f" => Value::Bool(false),
            _ => Value::String(text.into_owned()),
        },
        20 | 21 | 23 | 26 => text // int8/int2/int4/oid
            .parse::<i64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::String(text.into_owned())),
        700 | 701 => text // float4/float8
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(text.into_owned())),
        114 | 3802 => {
            // json / jsonb — parse so the host can show it as an interactive tree
            serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text.into_owned()))
        }
        _ => Value::String(text.into_owned()),
    }
}

/// Human-readable type name for the common Postgres OIDs.
fn type_name(oid: Oid) -> String {
    let name = match oid {
        16 => "bool",
        17 => "bytea",
        18 => "char",
        19 => "name",
        20 => "int8",
        21 => "int2",
        23 => "int4",
        25 => "text",
        26 => "oid",
        114 => "json",
        142 => "xml",
        700 => "float4",
        701 => "float8",
        790 => "money",
        869 => "inet",
        1042 => "bpchar",
        1043 => "varchar",
        1082 => "date",
        1083 => "time",
        1114 => "timestamp",
        1184 => "timestamptz",
        1186 => "interval",
        1266 => "timetz",
        1700 => "numeric",
        2950 => "uuid",
        3802 => "jsonb",
        _ => return format!("oid:{oid}"),
    };
    name.to_string()
}

fn error_message(body: &backend::ErrorResponseBody) -> String {
    let mut fields = body.fields();
    while let Ok(Some(f)) = fields.next() {
        if f.type_() == b'M' {
            return String::from_utf8_lossy(f.value_bytes()).into_owned();
        }
    }
    "server error".to_string()
}

fn flush(conn: &mut TcpShim, out: &mut BytesMut) -> Result<(), String> {
    conn.write_all(out).map_err(|e| e.to_string())?;
    conn.flush().map_err(|e| e.to_string())?;
    out.clear();
    Ok(())
}

/// Read backend messages, pulling more bytes from the socket until one frames.
fn read_message(
    conn: &mut TcpShim,
    inbuf: &mut BytesMut,
    scratch: &mut [u8],
) -> Result<backend::Message, String> {
    loop {
        if let Some(msg) = backend::Message::parse(inbuf).map_err(|e| e.to_string())? {
            return Ok(msg);
        }
        let n = conn.read(scratch).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("connection closed by server".to_string());
        }
        inbuf.extend_from_slice(&scratch[..n]);
    }
}
