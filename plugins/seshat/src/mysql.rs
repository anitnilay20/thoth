//! Minimal MySQL / MariaDB client over the host `tcp-client` shim. The wire
//! protocol is hand-rolled (no async, no std::net) — enough for the text
//! (`COM_QUERY`) protocol and `information_schema` introspection.
//!
//! Auth supported: `mysql_native_password` (SHA1 scramble) and
//! `caching_sha2_password` (SHA256 scramble fast path, plus full auth). Full
//! auth sends the cleartext password over TLS; without TLS it fetches the
//! server's RSA public key and sends the password RSA/OAEP-encrypted (see
//! `full_auth_sha2` / `rsa_encrypt_password`), so non-TLS connections work too.

use std::io::{Read, Write};

use serde_json::Value;
use sha1::{Digest, Sha1};
use sha2::Sha256;

use crate::db::{Column, ColumnInfo, DbAdapter, Profile, QueryResult, TableInfo};
use crate::shim::TcpShim;

// ── capability flags (subset we use) ────────────────────────────────────────
const CLIENT_LONG_PASSWORD: u32 = 0x0000_0001;
const CLIENT_LONG_FLAG: u32 = 0x0000_0004;
const CLIENT_CONNECT_WITH_DB: u32 = 0x0000_0008;
const CLIENT_PROTOCOL_41: u32 = 0x0000_0200;
const CLIENT_SSL: u32 = 0x0000_0800;
const CLIENT_TRANSACTIONS: u32 = 0x0000_2000;
const CLIENT_SECURE_CONNECTION: u32 = 0x0000_8000;
const CLIENT_PLUGIN_AUTH: u32 = 0x0008_0000;

const CHARSET_UTF8MB4: u8 = 45;
const MAX_PACKET: u32 = 16 * 1024 * 1024;

/// MySQL implementation of [`DbAdapter`].
pub struct Mysql;

impl DbAdapter for Mysql {
    fn connection_defaults(&self) -> crate::db::ConnectionDefaults {
        crate::db::ConnectionDefaults {
            port: 3306,
            user: "root",
            database: "",
            database_placeholder: "mysql",
        }
    }

    fn test_connection(&self, p: &Profile) -> Result<String, String> {
        let qr = run_query(p, "SELECT VERSION()")?;
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
            "SELECT schema_name FROM information_schema.schemata ORDER BY schema_name",
        )?;
        Ok(qr.rows.iter().map(|r| str_at(r, 0)).collect())
    }

    /// MySQL has no schema layer within a database, so the single "schema" is the
    /// database itself (the dispatch sets `p.database` to the target) — this keeps
    /// the shared Database → Schema → Table tree working.
    fn list_schemas(&self, p: &Profile) -> Result<Vec<String>, String> {
        Ok(vec![p.database.clone()])
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
                kind: if str_at(r, 1).eq_ignore_ascii_case("VIEW") {
                    "view"
                } else {
                    "table"
                }
                .to_string(),
            })
            .collect())
    }

    fn find_tables(&self, p: &Profile, query: &str) -> Result<Vec<TableInfo>, String> {
        // MySQL's `information_schema` spans every database on the server, so a
        // single connection can search them all — filter to matching tables in
        // any non-system database (results carry their database as the schema).
        let pattern = quote_literal(&format!("%{query}%"));
        // Order by table name first so matches interleave across databases
        // (rather than filling the 200-row cap with the first DB alphabetically).
        let sql = format!(
            "SELECT table_schema, table_name, table_type FROM information_schema.tables \
             WHERE table_schema NOT IN ('information_schema', 'mysql', 'performance_schema', 'sys') \
             AND table_name LIKE {pattern} \
             ORDER BY table_name, table_schema LIMIT 200"
        );
        let qr = run_query(p, &sql)?;
        Ok(qr
            .rows
            .iter()
            .map(|r| {
                // In MySQL the schema *is* the database.
                let db = str_at(r, 0);
                TableInfo {
                    database: Some(db.clone()),
                    schema: db,
                    name: str_at(r, 1),
                    kind: if str_at(r, 2).eq_ignore_ascii_case("VIEW") {
                        "view"
                    } else {
                        "table"
                    }
                    .to_string(),
                }
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
            "SELECT column_name, data_type, is_nullable, column_default, column_key \
             FROM information_schema.columns \
             WHERE table_schema = {schema} AND table_name = {table} \
             ORDER BY ordinal_position",
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
                nullable: str_at(r, 2).eq_ignore_ascii_case("YES"),
                default: r.get(3).and_then(|v| v.as_str()).map(String::from),
                primary_key: str_at(r, 4) == "PRI",
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

        // Columns with key flags (PRI/UNI) + foreign-key target.
        let col_sql = format!(
            "SELECT c.column_name, c.data_type, c.is_nullable, c.column_default, c.column_key, \
                    k.referenced_table_name, k.referenced_column_name \
             FROM information_schema.columns c \
             LEFT JOIN information_schema.key_column_usage k \
               ON k.table_schema = c.table_schema AND k.table_name = c.table_name \
              AND k.column_name = c.column_name AND k.referenced_table_name IS NOT NULL \
             WHERE c.table_schema = {s} AND c.table_name = {t} \
             ORDER BY c.ordinal_position"
        );
        let columns = run_query(p, &col_sql)?
            .rows
            .iter()
            .map(|r| {
                let key = str_at(r, 4);
                let fk = match (
                    r.get(5).and_then(|v| v.as_str()),
                    r.get(6).and_then(|v| v.as_str()),
                ) {
                    (Some(rt), Some(rc)) => Some(format!("{rt}.{rc}")),
                    _ => None,
                };
                ColumnInfo {
                    name: str_at(r, 0),
                    data_type: str_at(r, 1),
                    nullable: str_at(r, 2).eq_ignore_ascii_case("YES"),
                    default: r.get(3).and_then(|v| v.as_str()).map(String::from),
                    primary_key: key == "PRI",
                    unique: key == "UNI",
                    foreign_key: fk,
                }
            })
            .collect();

        // Indexes (non-fatal): group index rows by name, ordered by seq_in_index.
        let idx_sql = format!(
            "SELECT index_name, MIN(non_unique) AS non_unique, \
                    GROUP_CONCAT(column_name ORDER BY seq_in_index SEPARATOR ',') AS cols \
             FROM information_schema.statistics \
             WHERE table_schema = {s} AND table_name = {t} \
             GROUP BY index_name ORDER BY index_name"
        );
        let indexes = run_query(p, &idx_sql)
            .map(|qr| {
                qr.rows
                    .iter()
                    .map(|r| crate::db::IndexInfo {
                        name: str_at(r, 0),
                        unique: int_at(r, 1) == 0,
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
            "SELECT table_rows, data_length + index_length FROM information_schema.tables \
             WHERE table_schema = {s} AND table_name = {t}"
        );
        let (row_estimate, size) = run_query(p, &stat_sql)
            .ok()
            .and_then(|qr| qr.rows.into_iter().next())
            .map(|r| (int_at(&r, 0).max(0), human_size(int_at(&r, 1))))
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

fn str_at(row: &[Value], i: usize) -> String {
    row.get(i)
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_default()
}

/// Read an integer cell, tolerating either a JSON number or a numeric string.
fn int_at(row: &[Value], i: usize) -> i64 {
    row.get(i)
        .and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        })
        .unwrap_or(0)
}

/// Format a byte count as a compact human-readable size (e.g. `318 MB`).
fn human_size(bytes: i64) -> String {
    if bytes <= 0 {
        return "0 B".to_string();
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
        format!("{size:.0} {}", UNITS[unit])
    }
}

/// Quote a string as a MySQL SQL literal (backslash + quote escaping).
fn quote_literal(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "''"))
}

// ── connection / query ──────────────────────────────────────────────────────

/// Connect, authenticate, run `sql` via `COM_QUERY`, and return the text result.
fn run_query(p: &Profile, sql: &str) -> Result<QueryResult, String> {
    let mut conn = TcpShim::connect(&p.host, p.port, false).map_err(|e| e.to_string())?;
    connect_and_auth(&mut conn, p)?;

    // COM_QUERY resets the sequence to 0.
    let mut payload = vec![0x03u8];
    payload.extend_from_slice(sql.as_bytes());
    write_packet(&mut conn, 0, &payload)?;

    let (_, first) = read_packet(&mut conn)?;
    match first.first() {
        Some(0x00) => {
            // OK packet — statement with no result set (INSERT/UPDATE/DDL).
            let ok = parse_ok(&first);
            Ok(QueryResult {
                columns: Vec::new(),
                rows: Vec::new(),
                tag: Some(ok),
            })
        }
        Some(0xff) => Err(parse_err(&first)),
        Some(0xfb) => Err("LOCAL INFILE is not supported".to_string()),
        _ => read_result_set(&mut conn, &first),
    }
}

/// Read column definitions + text rows following a result-set header packet.
fn read_result_set(conn: &mut TcpShim, header: &[u8]) -> Result<QueryResult, String> {
    let mut cur = Cursor::new(header);
    let col_count = cur.lenenc_int()? as usize;

    let mut columns = Vec::with_capacity(col_count);
    let mut types = Vec::with_capacity(col_count);
    for _ in 0..col_count {
        let (_, pkt) = read_packet(conn)?;
        let (name, ty) = parse_column_def(&pkt)?;
        columns.push(Column {
            name,
            type_name: type_name(ty),
        });
        types.push(ty);
    }
    // EOF after the column definitions (we don't set CLIENT_DEPRECATE_EOF).
    expect_eof(conn)?;

    let mut rows = Vec::new();
    loop {
        let (_, pkt) = read_packet(conn)?;
        match pkt.first() {
            Some(0xfe) if pkt.len() < 9 => break, // EOF: end of rows
            Some(0xff) => return Err(parse_err(&pkt)),
            _ => rows.push(parse_text_row(&pkt, &types, col_count)?),
        }
    }
    Ok(QueryResult {
        columns,
        rows,
        tag: None,
    })
}

/// Read one packet expecting an EOF marker (`0xfe`, short packet).
fn expect_eof(conn: &mut TcpShim) -> Result<(), String> {
    let (_, pkt) = read_packet(conn)?;
    match pkt.first() {
        Some(0xfe) if pkt.len() < 9 => Ok(()),
        Some(0xff) => Err(parse_err(&pkt)),
        _ => Err("expected EOF after column definitions".to_string()),
    }
}

// ── handshake + auth ─────────────────────────────────────────────────────────

fn connect_and_auth(conn: &mut TcpShim, p: &Profile) -> Result<(), String> {
    let (seq, hs) = read_packet(conn)?;
    let (salt, server_caps, plugin) = parse_handshake(&hs)?;

    let mut caps = CLIENT_PROTOCOL_41
        | CLIENT_SECURE_CONNECTION
        | CLIENT_PLUGIN_AUTH
        | CLIENT_TRANSACTIONS
        | CLIENT_LONG_PASSWORD
        | CLIENT_LONG_FLAG;
    if !p.database.is_empty() {
        caps |= CLIENT_CONNECT_WITH_DB;
    }

    let mut seq = seq;
    if p.tls {
        if server_caps & CLIENT_SSL == 0 {
            return Err("server does not support TLS, but TLS was requested".to_string());
        }
        caps |= CLIENT_SSL;
        // SSL request: the caps/charset header only, then upgrade the stream.
        let mut ssl = Vec::new();
        put_u32(&mut ssl, caps);
        put_u32(&mut ssl, MAX_PACKET);
        ssl.push(CHARSET_UTF8MB4);
        ssl.extend_from_slice(&[0u8; 23]);
        seq += 1;
        write_packet(conn, seq, &ssl)?;
        conn.start_tls(&p.host).map_err(|e| e.to_string())?;
    }

    // Pick the auth method from the server's default plugin.
    let auth = scramble(&plugin, p.password.as_bytes(), &salt);
    seq += 1;
    write_packet(conn, seq, &handshake_response(caps, p, &plugin, &auth))?;

    // Auth exchange: OK / ERR / auth-switch / caching_sha2 more-data.
    auth_loop(conn, p, salt)
}

fn auth_loop(conn: &mut TcpShim, p: &Profile, salt: Vec<u8>) -> Result<(), String> {
    // The nonce used by the current auth method; updated on an auth switch.
    let mut salt = salt;
    loop {
        let (seq, pkt) = read_packet(conn)?;
        match pkt.first() {
            Some(0x00) => return Ok(()), // OK — authenticated
            Some(0xff) => return Err(parse_err(&pkt)),
            Some(0xfe) => {
                // Auth switch request: 0xfe, NUL plugin name, then a fresh salt.
                let mut cur = Cursor::new(&pkt[1..]);
                let plugin = cur.nul_str();
                salt = cur.rest();
                let auth = scramble(&plugin, p.password.as_bytes(), &salt);
                write_packet(conn, seq + 1, &auth)?;
            }
            Some(0x01) => {
                // caching_sha2_password fast/full-auth signal (0x01, status).
                match pkt.get(1) {
                    Some(0x03) => {} // fast auth success — next packet is OK
                    Some(0x04) => full_auth_sha2(conn, p, &salt, seq)?,
                    _ => return Err("unexpected caching_sha2_password state".to_string()),
                }
            }
            _ => return Err("unexpected packet during authentication".to_string()),
        }
    }
}

/// `caching_sha2_password` full authentication. Over TLS the server accepts the
/// cleartext password; otherwise the password is RSA-encrypted with the server's
/// public key (fetched on demand) before sending.
fn full_auth_sha2(conn: &mut TcpShim, p: &Profile, salt: &[u8], seq: u8) -> Result<(), String> {
    if p.tls {
        let mut cleartext = p.password.as_bytes().to_vec();
        cleartext.push(0);
        return write_packet(conn, seq + 1, &cleartext);
    }
    // Request the server's RSA public key (0x02), then encrypt with it.
    write_packet(conn, seq + 1, &[0x02])?;
    let (seq2, pk) = read_packet(conn)?;
    if pk.first() != Some(&0x01) {
        return Err("expected RSA public key from server".to_string());
    }
    let encrypted = rsa_encrypt_password(&pk[1..], p.password.as_bytes(), salt)?;
    write_packet(conn, seq2 + 1, &encrypted)
}

/// RSA-OAEP(SHA1) encrypt `password` (NUL-terminated, XOR the salt) with the
/// server's PEM public key — the MySQL `caching_sha2_password` full-auth scheme.
fn rsa_encrypt_password(pem: &[u8], password: &[u8], salt: &[u8]) -> Result<Vec<u8>, String> {
    use rsa::pkcs8::DecodePublicKey;
    use rsa::{Oaep, RsaPublicKey};

    if salt.is_empty() {
        return Err("missing auth salt for RSA encryption".to_string());
    }
    let pem = std::str::from_utf8(pem).map_err(|_| "invalid public-key PEM".to_string())?;
    let key = RsaPublicKey::from_public_key_pem(pem.trim()).map_err(|e| e.to_string())?;

    let mut buf = password.to_vec();
    buf.push(0);
    for (i, b) in buf.iter_mut().enumerate() {
        *b ^= salt[i % salt.len()];
    }
    key.encrypt(&mut rand::rngs::OsRng, Oaep::new::<Sha1>(), &buf)
        .map_err(|e| e.to_string())
}

/// Scramble the password for the given auth plugin. Empty password → empty.
fn scramble(plugin: &str, password: &[u8], salt: &[u8]) -> Vec<u8> {
    if password.is_empty() {
        return Vec::new();
    }
    match plugin {
        "caching_sha2_password" => scramble_sha2(password, salt),
        // Default to the SHA1 scramble (mysql_native_password / mysql_old covered
        // by the switch flow otherwise).
        _ => scramble_native(password, salt),
    }
}

/// `SHA1(pass) XOR SHA1(salt + SHA1(SHA1(pass)))`.
fn scramble_native(password: &[u8], salt: &[u8]) -> Vec<u8> {
    let stage1 = Sha1::digest(password);
    let stage2 = Sha1::digest(stage1);
    let mut concat = salt.to_vec();
    concat.extend_from_slice(&stage2);
    let seed = Sha1::digest(&concat);
    stage1.iter().zip(seed.iter()).map(|(a, b)| a ^ b).collect()
}

/// `SHA256(pass) XOR SHA256(SHA256(SHA256(pass)) + salt)`.
fn scramble_sha2(password: &[u8], salt: &[u8]) -> Vec<u8> {
    let d1 = Sha256::digest(password);
    let d2 = Sha256::digest(d1);
    let mut concat = d2.to_vec();
    concat.extend_from_slice(salt);
    let d3 = Sha256::digest(&concat);
    d1.iter().zip(d3.iter()).map(|(a, b)| a ^ b).collect()
}

/// Parse the initial handshake: returns `(salt, server_capabilities, plugin)`.
fn parse_handshake(pkt: &[u8]) -> Result<(Vec<u8>, u32, String), String> {
    let mut cur = Cursor::new(pkt);
    let proto = cur.u8()?;
    if proto != 10 {
        return Err(format!("unsupported MySQL protocol version {proto}"));
    }
    let _server_version = cur.nul_str();
    cur.skip(4); // connection id
    let mut salt = cur.bytes(8)?.to_vec(); // auth-plugin-data part 1
    cur.skip(1); // filler
    let cap_lower = cur.u16()? as u32;
    if cur.remaining() == 0 {
        return Err("truncated handshake".to_string());
    }
    let _charset = cur.u8()?;
    cur.skip(2); // status flags
    let cap_upper = (cur.u16()? as u32) << 16;
    let caps = cap_lower | cap_upper;
    let auth_data_len = cur.u8()? as usize;
    cur.skip(10); // reserved
    if caps & CLIENT_SECURE_CONNECTION != 0 {
        // part 2: at least 13 bytes; the last is a NUL terminator.
        let take = auth_data_len.saturating_sub(8).max(13);
        let part2 = cur.bytes(take.min(cur.remaining()))?;
        salt.extend_from_slice(part2);
        while salt.last() == Some(&0) {
            salt.pop();
        }
    }
    let plugin = if caps & CLIENT_PLUGIN_AUTH != 0 {
        cur.nul_str()
    } else {
        "mysql_native_password".to_string()
    };
    Ok((salt, caps, plugin))
}

/// Build the protocol-41 handshake response packet.
fn handshake_response(caps: u32, p: &Profile, plugin: &str, auth: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    put_u32(&mut out, caps);
    put_u32(&mut out, MAX_PACKET);
    out.push(CHARSET_UTF8MB4);
    out.extend_from_slice(&[0u8; 23]);
    out.extend_from_slice(p.user.as_bytes());
    out.push(0);
    // CLIENT_SECURE_CONNECTION: 1-byte length-prefixed auth response.
    out.push(auth.len() as u8);
    out.extend_from_slice(auth);
    if caps & CLIENT_CONNECT_WITH_DB != 0 {
        out.extend_from_slice(p.database.as_bytes());
        out.push(0);
    }
    if caps & CLIENT_PLUGIN_AUTH != 0 {
        out.extend_from_slice(plugin.as_bytes());
        out.push(0);
    }
    out
}

// ── packet parsing ────────────────────────────────────────────────────────────

/// Parse a column-definition packet → `(name, column_type)`.
fn parse_column_def(pkt: &[u8]) -> Result<(String, u8), String> {
    let mut cur = Cursor::new(pkt);
    cur.skip_lenenc_str()?; // catalog
    cur.skip_lenenc_str()?; // schema
    cur.skip_lenenc_str()?; // table
    cur.skip_lenenc_str()?; // org_table
    let name = cur.lenenc_str()?;
    cur.skip_lenenc_str()?; // org_name
    cur.skip_lenenc_int()?; // length of fixed fields (0x0c)
    cur.skip(2); // charset
    cur.skip(4); // column length
    let ty = cur.u8()?;
    // 2-byte little-endian column flags follow the type byte. ENUM columns are
    // sent as MYSQL_TYPE_STRING with the ENUM_FLAG (0x0100) set, so surface them
    // as the enum type for type-aware rendering.
    let flags = cur.u8()? as u16 | ((cur.u8()? as u16) << 8);
    let ty = if flags & 0x0100 != 0 { 247 } else { ty };
    Ok((name, ty))
}

/// Parse a text-protocol row: each column is a length-encoded string or NULL.
fn parse_text_row(pkt: &[u8], types: &[u8], n: usize) -> Result<Vec<Value>, String> {
    let mut cur = Cursor::new(pkt);
    let mut values = Vec::with_capacity(n);
    for &ty in types.iter().take(n) {
        if cur.peek() == Some(0xfb) {
            cur.skip(1);
            values.push(Value::Null);
        } else {
            let bytes = cur.lenenc_bytes()?;
            values.push(decode_value(ty, bytes));
        }
    }
    Ok(values)
}

/// Decode a text-format cell to native JSON for the common types; string else.
fn decode_value(ty: u8, bytes: &[u8]) -> Value {
    let text = String::from_utf8_lossy(bytes);
    match ty {
        // TINY, SHORT, LONG, LONGLONG, INT24, YEAR
        1 | 2 | 3 | 8 | 9 | 13 => text
            .parse::<i64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::String(text.into_owned())),
        // FLOAT, DOUBLE
        4 | 5 => text
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(text.into_owned())),
        // JSON
        245 => serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text.into_owned())),
        // DECIMAL/NEWDECIMAL kept as strings to preserve precision; everything
        // else (strings, dates, blobs) is text.
        _ => Value::String(text.into_owned()),
    }
}

/// Human-readable type name for the common MySQL column type codes.
fn type_name(ty: u8) -> String {
    let name = match ty {
        0 | 246 => "decimal",
        1 => "tinyint",
        2 => "smallint",
        3 => "int",
        4 => "float",
        5 => "double",
        7 => "timestamp",
        8 => "bigint",
        9 => "mediumint",
        10 => "date",
        11 => "time",
        12 => "datetime",
        13 => "year",
        15 | 253 => "varchar",
        16 => "bit",
        245 => "json",
        247 => "enum",
        249 => "tinytext",
        250 => "mediumtext",
        251 => "longtext",
        252 => "text",
        254 => "char",
        _ => return format!("type:{ty}"),
    };
    name.to_string()
}

/// Parse an OK packet into a short command tag (e.g. `OK · 3 rows`).
fn parse_ok(pkt: &[u8]) -> String {
    let mut cur = Cursor::new(&pkt[1..]);
    let affected = cur.lenenc_int().unwrap_or(0);
    format!(
        "OK · {affected} row{}",
        if affected == 1 { "" } else { "s" }
    )
}

/// Parse an ERR packet into its message.
fn parse_err(pkt: &[u8]) -> String {
    // 0xff, 2-byte error code, ('#' + 5-byte sqlstate) when protocol-41, message.
    let mut cur = Cursor::new(&pkt[1..]);
    let code = cur.u16().unwrap_or(0);
    if cur.peek() == Some(b'#') {
        cur.skip(6); // '#' + sqlstate
    }
    let msg = String::from_utf8_lossy(&cur.rest()).into_owned();
    if msg.is_empty() {
        format!("MySQL error {code}")
    } else {
        msg
    }
}

// ── framing ─────────────────────────────────────────────────────────────────

fn read_packet(conn: &mut TcpShim) -> Result<(u8, Vec<u8>), String> {
    let mut hdr = [0u8; 4];
    conn.read_exact(&mut hdr).map_err(|e| e.to_string())?;
    let len = (hdr[0] as usize) | ((hdr[1] as usize) << 8) | ((hdr[2] as usize) << 16);
    let seq = hdr[3];
    let mut payload = vec![0u8; len];
    if len > 0 {
        conn.read_exact(&mut payload).map_err(|e| e.to_string())?;
    }
    Ok((seq, payload))
}

fn write_packet(conn: &mut TcpShim, seq: u8, payload: &[u8]) -> Result<(), String> {
    let len = payload.len();
    let hdr = [len as u8, (len >> 8) as u8, (len >> 16) as u8, seq];
    conn.write_all(&hdr).map_err(|e| e.to_string())?;
    conn.write_all(payload).map_err(|e| e.to_string())?;
    conn.flush().map_err(|e| e.to_string())
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

// ── payload cursor ────────────────────────────────────────────────────────────

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }
    fn peek(&self) -> Option<u8> {
        self.buf.get(self.pos).copied()
    }
    fn skip(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.buf.len());
    }
    fn u8(&mut self) -> Result<u8, String> {
        let v = *self.buf.get(self.pos).ok_or("unexpected end of packet")?;
        self.pos += 1;
        Ok(v)
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.pos + n > self.buf.len() {
            return Err("unexpected end of packet".to_string());
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn rest(&mut self) -> Vec<u8> {
        let s = self.buf[self.pos.min(self.buf.len())..].to_vec();
        self.pos = self.buf.len();
        s
    }
    fn nul_str(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.buf.len() && self.buf[self.pos] != 0 {
            self.pos += 1;
        }
        let s = String::from_utf8_lossy(&self.buf[start..self.pos]).into_owned();
        if self.pos < self.buf.len() {
            self.pos += 1; // consume NUL
        }
        s
    }
    fn lenenc_int(&mut self) -> Result<u64, String> {
        match self.u8()? {
            v @ 0..=0xfa => Ok(v as u64),
            0xfc => Ok(self.u16()? as u64),
            0xfd => {
                let b = self.bytes(3)?;
                Ok((b[0] as u64) | ((b[1] as u64) << 8) | ((b[2] as u64) << 16))
            }
            0xfe => {
                let b = self.bytes(8)?;
                Ok(u64::from_le_bytes(b.try_into().unwrap()))
            }
            other => Err(format!(
                "invalid length-encoded integer prefix 0x{other:02x}"
            )),
        }
    }
    fn skip_lenenc_int(&mut self) -> Result<(), String> {
        self.lenenc_int()?;
        Ok(())
    }
    fn lenenc_bytes(&mut self) -> Result<&'a [u8], String> {
        let n = self.lenenc_int()? as usize;
        self.bytes(n)
    }
    fn lenenc_str(&mut self) -> Result<String, String> {
        Ok(String::from_utf8_lossy(self.lenenc_bytes()?).into_owned())
    }
    fn skip_lenenc_str(&mut self) -> Result<(), String> {
        let n = self.lenenc_int()? as usize;
        self.skip(n);
        Ok(())
    }
}
