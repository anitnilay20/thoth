//! Minimal Postgres client over the host `tcp-client` shim, using the
//! `postgres-protocol` codec (no async, no std::net). Simple-query protocol
//! only — enough for the M0 spike (`SELECT 1`) and basic SELECTs.
//!
//! Auth supported: trust, cleartext, md5, SCRAM-SHA-256 (the stock `postgres`
//! default). Values come back in text format and are surfaced as JSON strings
//! (or null); richer typing lands in later phases.

use std::io::{Read, Write};

use bytes::BytesMut;
use fallible_iterator::FallibleIterator;
use postgres_protocol::authentication;
use postgres_protocol::authentication::sasl::{ChannelBinding, ScramSha256};
use postgres_protocol::message::{backend, frontend};
use serde_json::{Map, Value};

use crate::shim::TcpShim;

pub struct Profile {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    pub tls: bool,
}

const READ_CHUNK: usize = 16 * 1024;

/// Connect, run `sql` via the simple-query protocol, return rows as a JSON array
/// of `{column: value}` objects. Blocking — runs on the host query worker.
pub fn run_query(p: &Profile, sql: &str) -> Result<Value, String> {
    let mut conn = TcpShim::connect(&p.host, p.port, p.tls).map_err(|e| e.to_string())?;
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

    let mut columns: Vec<String> = Vec::new();
    let mut rows: Vec<Value> = Vec::new();
    loop {
        match read_message(&mut conn, &mut inbuf, &mut scratch)? {
            backend::Message::RowDescription(body) => {
                columns = body
                    .fields()
                    .map(|f| Ok(f.name().to_string()))
                    .collect()
                    .map_err(|e: std::io::Error| e.to_string())?;
            }
            backend::Message::DataRow(body) => rows.push(row_to_json(&columns, &body)?),
            backend::Message::ErrorResponse(body) => return Err(error_message(&body)),
            backend::Message::ReadyForQuery(_) => break,
            _ => {} // CommandComplete, EmptyQueryResponse, NoticeResponse, etc.
        }
    }

    frontend::terminate(&mut out);
    let _ = flush(&mut conn, &mut out);
    Ok(Value::Array(rows))
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

fn row_to_json(columns: &[String], body: &backend::DataRowBody) -> Result<Value, String> {
    let ranges: Vec<Option<std::ops::Range<usize>>> = body
        .ranges()
        .collect()
        .map_err(|e: std::io::Error| e.to_string())?;
    let buf = body.buffer();
    let mut obj = Map::new();
    for (i, range) in ranges.into_iter().enumerate() {
        let name = columns.get(i).cloned().unwrap_or_else(|| i.to_string());
        let value = match range {
            Some(r) => Value::String(String::from_utf8_lossy(&buf[r]).into_owned()),
            None => Value::Null,
        };
        obj.insert(name, value);
    }
    Ok(Value::Object(obj))
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
