//! MongoDB wire protocol (`OP_MSG`) + BSON codec over `TcpShim` (issue #101).
//!
//! Supports:
//!   * `OP_MSG` (opcode 2013) request/response framing
//!   * BSON document/array/scalar encoding & decoding (UTF-8, int32, int64,
//!     double, bool, null, ObjectId, UTC datetime)
//!   * SCRAM-SHA-256 authentication (`saslStart` / `saslContinue`)
//!   * Schema discovery (`listDatabases`, `listCollections`, document field
//!     inference) and `find` queries (JSON filter or `SELECT * FROM col`).

use std::collections::BTreeMap;
use std::io::{Read, Write};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::db::{
    AuthMode, Column, ColumnInfo, ConnectionDefaults, DbAdapter, Profile, QueryResult, TableDetail,
    TableInfo,
};
use crate::shim::TcpShim;

const OP_MSG: i32 = 2013;

/// MongoDB implementation of [`DbAdapter`].
pub struct MongoDb;

impl DbAdapter for MongoDb {
    fn connection_defaults(&self) -> ConnectionDefaults {
        ConnectionDefaults {
            port: 27017,
            user: "",
            database: "admin",
            database_placeholder: "admin",
        }
    }

    fn test_connection(&self, p: &Profile) -> Result<String, String> {
        let mut conn = connect_and_auth(p)?;
        let res = run_cmd(&mut conn, "admin", &json!({ "buildInfo": 1 }))?;
        let ver = res
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("connected");
        Ok(format!("MongoDB {ver}"))
    }

    fn list_databases(&self, p: &Profile) -> Result<Vec<String>, String> {
        let mut conn = connect_and_auth(p)?;
        let res = run_cmd(&mut conn, "admin", &json!({ "listDatabases": 1, "nameOnly": true }))?;
        let mut out: Vec<String> = res
            .get("databases")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|d| d.get("name").and_then(Value::as_str).map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        Ok(out)
    }

    fn list_schemas(&self, p: &Profile) -> Result<Vec<String>, String> {
        let db = active_db(p);
        Ok(vec![db.to_string()])
    }

    fn list_tables(&self, p: &Profile, schema: &str) -> Result<Vec<TableInfo>, String> {
        let mut conn = connect_and_auth(p)?;
        let db = if schema.is_empty() { active_db(p) } else { schema };
        let res = run_cmd(&mut conn, db, &json!({ "listCollections": 1 }))?;
        let mut out: Vec<TableInfo> = res
            .get("cursor")
            .and_then(|c| c.get("firstBatch"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let name = c.get("name").and_then(Value::as_str)?.to_string();
                        let kind = c
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("collection")
                            .to_string();
                        Some(TableInfo {
                            database: Some(db.to_string()),
                            schema: db.to_string(),
                            name,
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
        let tables = self.list_tables(p, active_db(p))?;
        Ok(tables
            .into_iter()
            .filter(|t| needle.is_empty() || t.name.to_lowercase().contains(&needle))
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
        let mut conn = connect_and_auth(p)?;
        let db = if schema.is_empty() { active_db(p) } else { schema };
        let res = run_cmd(
            &mut conn,
            db,
            &json!({ "find": table, "limit": 20 }),
        )?;
        let docs = extract_batch(&res);
        let mut field_types: BTreeMap<String, &'static str> = BTreeMap::new();
        for doc in &docs {
            if let Some(obj) = doc.as_object() {
                for (k, v) in obj {
                    field_types.entry(k.clone()).or_insert_with(|| json_type_name(v));
                }
            }
        }
        let columns = field_types
            .into_iter()
            .map(|(name, ty)| ColumnInfo {
                primary_key: name == "_id",
                name,
                data_type: ty.to_string(),
                nullable: true,
                default: None,
                unique: false,
                foreign_key: None,
            })
            .collect();

        Ok(TableDetail {
            columns,
            indexes: Vec::new(),
            row_estimate: docs.len() as i64,
            size: String::new(),
        })
    }

    fn run_query(&self, p: &Profile, sql: &str) -> Result<QueryResult, String> {
        let mut conn = connect_and_auth(p)?;
        let db = active_db(p);
        let cmd = parse_mongo_query(sql)?;
        let res = run_cmd(&mut conn, db, &cmd)?;

        if res.get("cursor").is_some() {
            let docs = extract_batch(&res);
            return Ok(docs_to_query_result(&docs));
        }

        // Command response (e.g. buildInfo, count, ping).
        Ok(docs_to_query_result(&[res]))
    }
}

fn active_db(p: &Profile) -> &str {
    let db = p.database.trim();
    if db.is_empty() {
        "admin"
    } else {
        db
    }
}

fn json_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_i64() || n.is_u64() => "int64",
        Value::Number(_) => "double",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Parse either a JSON command/find object, a bare collection name, or
/// `SELECT * FROM <collection> [LIMIT n]`.
pub fn parse_mongo_query(input: &str) -> Result<Value, String> {
    let trimmed = input.trim().trim_end_matches(';').trim();
    if trimmed.is_empty() {
        return Err("Empty MongoDB query".to_string());
    }
    if trimmed.starts_with('{') {
        let v: Value =
            serde_json::from_str(trimmed).map_err(|e| format!("Invalid JSON query: {e}"))?;
        if let Some(col) = v.get("collection").and_then(Value::as_str) {
            let filter = v.get("filter").cloned().unwrap_or_else(|| json!({}));
            let limit = v.get("limit").and_then(Value::as_i64).unwrap_or(100);
            return Ok(json!({ "find": col, "filter": filter, "limit": limit }));
        }
        return Ok(v);
    }

    let upper = trimmed.to_uppercase();
    if upper.starts_with("SELECT ") {
        if let Some(from_idx) = upper.find(" FROM ") {
            let after_from = trimmed[from_idx + 6..].trim();
            let mut parts = after_from.split_whitespace();
            if let Some(raw_col) = parts.next() {
                let col = raw_col
                    .split('.')
                    .last()
                    .unwrap_or(raw_col)
                    .trim_matches(|c| c == '"' || c == '`' || c == '\'');
                let mut limit = 100i64;
                while let Some(tok) = parts.next() {
                    if tok.eq_ignore_ascii_case("LIMIT") {
                        if let Some(n) = parts.next().and_then(|s| s.parse::<i64>().ok()) {
                            limit = n;
                        }
                    }
                }
                return Ok(json!({ "find": col, "filter": {}, "limit": limit }));
            }
        }
    }

    Ok(json!({ "find": trimmed, "filter": {}, "limit": 100 }))
}

fn extract_batch(res: &Value) -> Vec<Value> {
    res.get("cursor")
        .and_then(|c| c.get("firstBatch"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Convert a slice of JSON-decoded BSON documents into tabular [`QueryResult`].
pub fn docs_to_query_result(docs: &[Value]) -> QueryResult {
    let mut col_names: Vec<String> = Vec::new();
    for doc in docs {
        if let Some(obj) = doc.as_object() {
            for k in obj.keys() {
                if !col_names.iter().any(|c| c == k) {
                    col_names.push(k.clone());
                }
            }
        }
    }
    let columns: Vec<Column> = col_names
        .iter()
        .map(|name| Column {
            name: name.clone(),
            type_name: "bson".to_string(),
        })
        .collect();
    let rows = docs
        .iter()
        .map(|doc| {
            col_names
                .iter()
                .map(|c| doc.get(c).cloned().unwrap_or(Value::Null))
                .collect()
        })
        .collect();
    QueryResult {
        columns,
        rows,
        tag: Some(format!("{} doc(s)", docs.len())),
    }
}

fn connect_and_auth(p: &Profile) -> Result<TcpShim, String> {
    let mut conn = TcpShim::connect(&p.host, p.port, p.tls).map_err(|e| e.to_string())?;
    if p.auth != AuthMode::None && !p.user.trim().is_empty() {
        scram_sha256_auth(&mut conn, active_db(p), &p.user, &p.password)?;
    }
    Ok(conn)
}

fn run_cmd(conn: &mut TcpShim, db: &str, cmd: &Value) -> Result<Value, String> {
    let mut obj = cmd
        .as_object()
        .cloned()
        .ok_or_else(|| "MongoDB command must be a JSON object".to_string())?;
    obj.insert("$db".to_string(), Value::String(db.to_string()));
    let bson = encode_bson_document(&Value::Object(obj))?;
    let frame = encode_op_msg(&bson, 1);
    conn.write_all(&frame).map_err(|e| e.to_string())?;
    conn.flush().map_err(|e| e.to_string())?;

    let reply_doc = read_op_msg_reply(conn)?;
    let ok = reply_doc
        .get("ok")
        .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
        .unwrap_or(1.0);
    if ok == 0.0 {
        let msg = reply_doc
            .get("errmsg")
            .and_then(Value::as_str)
            .unwrap_or("MongoDB command failed");
        return Err(msg.to_string());
    }
    Ok(reply_doc)
}

/// Frame a single-section `OP_MSG` (kind 0 body document).
pub fn encode_op_msg(bson_doc: &[u8], request_id: i32) -> Vec<u8> {
    let total_len = 16 + 4 + 1 + bson_doc.len();
    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(&(total_len as i32).to_le_bytes());
    out.extend_from_slice(&request_id.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes()); // responseTo
    out.extend_from_slice(&OP_MSG.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // flagBits
    out.push(0u8); // section kind 0 (body)
    out.extend_from_slice(bson_doc);
    out
}

fn read_op_msg_reply(conn: &mut TcpShim) -> Result<Value, String> {
    let mut hdr = [0u8; 16];
    conn.read_exact(&mut hdr).map_err(|e| e.to_string())?;
    let msg_len = i32::from_le_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]) as usize;
    let op_code = i32::from_le_bytes([hdr[12], hdr[13], hdr[14], hdr[15]]);
    if msg_len < 16 || msg_len > 64 * 1024 * 1024 {
        return Err(format!("Invalid MongoDB message length: {msg_len}"));
    }
    let mut body = vec![0u8; msg_len - 16];
    conn.read_exact(&mut body).map_err(|e| e.to_string())?;
    if op_code != OP_MSG {
        return Err(format!("Unexpected MongoDB opcode {op_code} (expected OP_MSG)"));
    }
    if body.len() < 5 || body[4] != 0 {
        return Err("Unsupported OP_MSG section layout".to_string());
    }
    let (val, _) = decode_bson_document(&body[5..])?;
    Ok(val)
}

// ── SCRAM-SHA-256 ────────────────────────────────────────────────────────────

fn scram_sha256_auth(
    conn: &mut TcpShim,
    db: &str,
    user: &str,
    password: &str,
) -> Result<(), String> {
    let client_nonce = "thoth-seshat-nonce-2026";
    let escaped_user = user.replace('=', "=3D").replace(',', "=2C");
    let client_first_bare = format!("n={escaped_user},r={client_nonce}");
    let client_first = format!("n,,{client_first_bare}");

    let start = run_cmd(
        conn,
        db,
        &json!({
            "saslStart": 1,
            "mechanism": "SCRAM-SHA-256",
            "payload": encode_b64_str(client_first.as_bytes()),
            "autoAuthorize": 1
        }),
    )?;

    let conv_id = start.get("conversationId").cloned().unwrap_or(json!(1));
    let server_first_b64 = start
        .get("payload")
        .and_then(Value::as_str)
        .ok_or_else(|| "Missing SCRAM server-first payload".to_string())?;
    let server_first_bytes = decode_b64_str(server_first_b64)?;
    let server_first =
        String::from_utf8(server_first_bytes).map_err(|e| format!("Invalid UTF-8 in SCRAM: {e}"))?;

    let mut r_val = "";
    let mut salt_b64 = "";
    let mut iters: u32 = 4096;
    for part in server_first.split(',') {
        if let Some(v) = part.strip_prefix("r=") {
            r_val = v;
        } else if let Some(v) = part.strip_prefix("s=") {
            salt_b64 = v;
        } else if let Some(v) = part.strip_prefix("i=") {
            iters = v.parse().unwrap_or(4096);
        }
    }
    let salt = decode_b64_str(salt_b64)?;
    let salted_pw = pbkdf2_sha256(password.as_bytes(), &salt, iters);
    let client_key = hmac_sha256(&salted_pw, b"Client Key");
    let stored_key = Sha256::digest(client_key);
    let client_final_no_proof = format!("c=biws,r={r_val}");
    let auth_msg = format!("{client_first_bare},{server_first},{client_final_no_proof}");
    let client_sig = hmac_sha256(&stored_key, auth_msg.as_bytes());
    let proof: Vec<u8> = client_key
        .iter()
        .zip(client_sig.iter())
        .map(|(a, b)| a ^ b)
        .collect();
    let client_final = format!("{client_final_no_proof},p={}", encode_b64_str(&proof));

    let cont = run_cmd(
        conn,
        db,
        &json!({
            "saslContinue": 1,
            "conversationId": conv_id,
            "payload": encode_b64_str(client_final.as_bytes())
        }),
    )?;

    if !cont.get("done").and_then(Value::as_bool).unwrap_or(false) {
        let _ = run_cmd(
            conn,
            db,
            &json!({
                "saslContinue": 1,
                "conversationId": conv_id,
                "payload": ""
            }),
        )?;
    }
    Ok(())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let d = Sha256::digest(key);
        k[..32].copy_from_slice(&d);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(data);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    outer.finalize().into()
}

fn pbkdf2_sha256(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut block = Vec::with_capacity(salt.len() + 4);
    block.extend_from_slice(salt);
    block.extend_from_slice(&1u32.to_be_bytes());
    let mut u = hmac_sha256(password, &block);
    let mut out = u;
    for _ in 1..iterations {
        u = hmac_sha256(password, &u);
        for i in 0..32 {
            out[i] ^= u[i];
        }
    }
    out
}

fn encode_b64_str(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn decode_b64_str(s: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| format!("Invalid base64: {e}"))
}

// ── Minimal BSON encoder / decoder ───────────────────────────────────────────

/// Encode a JSON object as a BSON document.
pub fn encode_bson_document(val: &Value) -> Result<Vec<u8>, String> {
    let obj = val
        .as_object()
        .ok_or_else(|| "BSON root must be an object".to_string())?;
    let mut body = Vec::new();
    for (k, v) in obj {
        encode_bson_element(&mut body, k, v)?;
    }
    body.push(0u8);
    let total = (4 + body.len()) as i32;
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

fn encode_bson_element(buf: &mut Vec<u8>, key: &str, val: &Value) -> Result<(), String> {
    match val {
        Value::Null => {
            buf.push(0x0A);
            buf.extend_from_slice(key.as_bytes());
            buf.push(0);
        }
        Value::Bool(b) => {
            buf.push(0x08);
            buf.extend_from_slice(key.as_bytes());
            buf.push(0);
            buf.push(u8::from(*b));
        }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if let Ok(i32v) = i32::try_from(i) {
                    buf.push(0x10);
                    buf.extend_from_slice(key.as_bytes());
                    buf.push(0);
                    buf.extend_from_slice(&i32v.to_le_bytes());
                } else {
                    buf.push(0x12);
                    buf.extend_from_slice(key.as_bytes());
                    buf.push(0);
                    buf.extend_from_slice(&i.to_le_bytes());
                }
            } else if let Some(f) = n.as_f64() {
                buf.push(0x01);
                buf.extend_from_slice(key.as_bytes());
                buf.push(0);
                buf.extend_from_slice(&f.to_le_bytes());
            }
        }
        Value::String(s) => {
            buf.push(0x02);
            buf.extend_from_slice(key.as_bytes());
            buf.push(0);
            let len = (s.len() + 1) as i32;
            buf.extend_from_slice(&len.to_le_bytes());
            buf.extend_from_slice(s.as_bytes());
            buf.push(0);
        }
        Value::Array(arr) => {
            buf.push(0x04);
            buf.extend_from_slice(key.as_bytes());
            buf.push(0);
            let mut inner = Vec::new();
            for (i, item) in arr.iter().enumerate() {
                encode_bson_element(&mut inner, &i.to_string(), item)?;
            }
            inner.push(0);
            buf.extend_from_slice(&((4 + inner.len()) as i32).to_le_bytes());
            buf.extend_from_slice(&inner);
        }
        Value::Object(_) => {
            buf.push(0x03);
            buf.extend_from_slice(key.as_bytes());
            buf.push(0);
            let sub = encode_bson_document(val)?;
            buf.extend_from_slice(&sub);
        }
    }
    Ok(())
}

/// Decode a BSON document from `data`, returning `(Value::Object, bytes_consumed)`.
pub fn decode_bson_document(data: &[u8]) -> Result<(Value, usize), String> {
    if data.len() < 5 {
        return Err("Truncated BSON document".to_string());
    }
    let doc_len = i32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if doc_len < 5 || doc_len > data.len() {
        return Err(format!("Invalid BSON document length: {doc_len}"));
    }
    let mut pos = 4;
    let end = doc_len - 1;
    let mut map = Map::new();
    while pos < end {
        let tag = data[pos];
        pos += 1;
        if tag == 0 {
            break;
        }
        let nul = data[pos..end]
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| "Unterminated BSON key".to_string())?;
        let key = String::from_utf8_lossy(&data[pos..pos + nul]).to_string();
        pos += nul + 1;
        let val = decode_bson_value(tag, data, &mut pos, end)?;
        map.insert(key, val);
    }
    Ok((Value::Object(map), doc_len))
}

fn decode_bson_value(tag: u8, data: &[u8], pos: &mut usize, end: usize) -> Result<Value, String> {
    match tag {
        0x01 => {
            let f = f64::from_le_bytes(take_bytes::<8>(data, pos, end)?);
            Ok(serde_json::Number::from_f64(f)
                .map(Value::Number)
                .unwrap_or(Value::Null))
        }
        0x02 | 0x0D | 0x0E => {
            let len = i32::from_le_bytes(take_bytes::<4>(data, pos, end)?) as usize;
            if len == 0 || *pos + len > end {
                return Err("Invalid BSON string length".to_string());
            }
            let s = String::from_utf8_lossy(&data[*pos..*pos + len - 1]).to_string();
            *pos += len;
            Ok(Value::String(s))
        }
        0x03 => {
            let (v, n) = decode_bson_document(&data[*pos..end])?;
            *pos += n;
            Ok(v)
        }
        0x04 => {
            let (v, n) = decode_bson_document(&data[*pos..end])?;
            *pos += n;
            let arr = v
                .as_object()
                .map(|m| m.values().cloned().collect())
                .unwrap_or_default();
            Ok(Value::Array(arr))
        }
        0x05 => {
            let len = i32::from_le_bytes(take_bytes::<4>(data, pos, end)?) as usize;
            *pos += 1; // subtype
            if *pos + len > end {
                return Err("Truncated BSON binary".to_string());
            }
            let b64 = encode_b64_str(&data[*pos..*pos + len]);
            *pos += len;
            Ok(Value::String(b64))
        }
        0x06 | 0x0A | 0x7F | 0xFF => Ok(Value::Null),
        0x07 => {
            let oid = take_bytes::<12>(data, pos, end)?;
            let hex: String = oid.iter().map(|b| format!("{b:02x}")).collect();
            Ok(Value::String(hex))
        }
        0x08 => {
            let b = take_bytes::<1>(data, pos, end)?[0] != 0;
            Ok(Value::Bool(b))
        }
        0x09 | 0x12 => {
            let i = i64::from_le_bytes(take_bytes::<8>(data, pos, end)?);
            Ok(Value::Number(i.into()))
        }
        0x10 => {
            let i = i32::from_le_bytes(take_bytes::<4>(data, pos, end)?);
            Ok(Value::Number(i.into()))
        }
        0x11 => {
            let u = u64::from_le_bytes(take_bytes::<8>(data, pos, end)?);
            Ok(Value::Number(u.into()))
        }
        other => Err(format!("Unsupported BSON type 0x{other:02x}")),
    }
}

fn take_bytes<const N: usize>(data: &[u8], pos: &mut usize, end: usize) -> Result<[u8; N], String> {
    if *pos + N > end {
        return Err("Truncated BSON field".to_string());
    }
    let mut buf = [0u8; N];
    buf.copy_from_slice(&data[*pos..*pos + N]);
    *pos += N;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bson_round_trips_document_and_array() {
        let doc = json!({
            "name": "thoth",
            "count": 42,
            "ratio": 1.5,
            "active": true,
            "tags": ["json", "bson"]
        });
        let encoded = encode_bson_document(&doc).unwrap();
        let (decoded, consumed) = decode_bson_document(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded["name"], json!("thoth"));
        assert_eq!(decoded["count"], json!(42));
        assert_eq!(decoded["ratio"], json!(1.5));
        assert_eq!(decoded["active"], json!(true));
        assert_eq!(decoded["tags"], json!(["json", "bson"]));
    }

    #[test]
    fn parses_select_and_json_mongo_queries() {
        let q1 = parse_mongo_query("SELECT * FROM users LIMIT 25").unwrap();
        assert_eq!(q1, json!({ "find": "users", "filter": {}, "limit": 25 }));

        let q2 = parse_mongo_query(r#"{"collection": "orders", "filter": {"status": "open"}, "limit": 10}"#).unwrap();
        assert_eq!(
            q2,
            json!({ "find": "orders", "filter": {"status": "open"}, "limit": 10 })
        );
    }
}
