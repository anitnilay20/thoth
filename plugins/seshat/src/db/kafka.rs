//! Apache Kafka binary wire-protocol client over `TcpShim` (issue #103).
//!
//! Implements core Kafka protocol requests:
//!   * `ApiVersions` (API key 18, v0)
//!   * `Metadata` (API key 3, v0) — brokers, topics, and partitions
//!   * `ListOffsets` (API key 2, v0) — earliest/latest offsets
//!   * `Fetch` (API key 1, v0) — consume records from a topic/partition
//!   * `SaslHandshake` (17) + `SaslAuthenticate` (36) for SASL/PLAIN
//!
//! Query text syntax:
//!   * `TOPICS` — list all topics & partition counts
//!   * `BROKERS` — list cluster brokers
//!   * `CONSUME <topic> [LIMIT <n>]` or `SELECT * FROM "<topic>"` — fetch recent messages

use std::io::{Read, Write};

use serde_json::json;

use crate::db::{
    AuthMode, Column, ColumnInfo, ConnectionDefaults, DbAdapter, Profile, QueryResult, TableDetail,
    TableInfo,
};
use crate::shim::TcpShim;

const API_FETCH: i16 = 1;
const API_LIST_OFFSETS: i16 = 2;
const API_METADATA: i16 = 3;
const API_SASL_HANDSHAKE: i16 = 17;
const API_API_VERSIONS: i16 = 18;
const API_SASL_AUTHENTICATE: i16 = 36;

/// Kafka implementation of [`DbAdapter`].
pub struct Kafka;

/// Decoded broker metadata from Kafka `Metadata` response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokerInfo {
    pub node_id: i32,
    pub host: String,
    pub port: i32,
}

/// Decoded topic metadata from Kafka `Metadata` response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopicInfo {
    pub name: String,
    pub partitions: Vec<PartitionInfo>,
}

/// Decoded partition metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartitionInfo {
    pub id: i32,
    pub leader: i32,
    pub replicas: Vec<i32>,
    pub isr: Vec<i32>,
}

impl DbAdapter for Kafka {
    fn connection_defaults(&self) -> ConnectionDefaults {
        ConnectionDefaults {
            port: 9092,
            user: "",
            database: "",
            database_placeholder: "cluster",
        }
    }

    fn test_connection(&self, p: &Profile) -> Result<String, String> {
        let mut conn = connect_and_auth(p)?;
        let (brokers, topics) = fetch_metadata(&mut conn)?;
        Ok(format!(
            "Kafka · {} broker(s), {} topic(s)",
            brokers.len(),
            topics.len()
        ))
    }

    fn list_databases(&self, _p: &Profile) -> Result<Vec<String>, String> {
        Ok(vec!["cluster".to_string()])
    }

    fn list_schemas(&self, _p: &Profile) -> Result<Vec<String>, String> {
        Ok(vec!["topics".to_string(), "brokers".to_string()])
    }

    fn list_tables(&self, p: &Profile, schema: &str) -> Result<Vec<TableInfo>, String> {
        let mut conn = connect_and_auth(p)?;
        let (brokers, topics) = fetch_metadata(&mut conn)?;
        if schema == "brokers" {
            return Ok(brokers
                .into_iter()
                .map(|b| TableInfo {
                    database: None,
                    schema: "brokers".to_string(),
                    name: format!("broker-{} ({}:{})", b.node_id, b.host, b.port),
                    kind: "broker".to_string(),
                })
                .collect());
        }
        let mut out: Vec<TableInfo> = topics
            .into_iter()
            .filter(|t| !t.name.starts_with("__"))
            .map(|t| TableInfo {
                database: None,
                schema: "topics".to_string(),
                name: t.name,
                kind: "topic".to_string(),
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    fn find_tables(&self, p: &Profile, query: &str) -> Result<Vec<TableInfo>, String> {
        let needle = query.trim().to_lowercase();
        let tables = self.list_tables(p, "topics")?;
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
        _schema: &str,
        table: &str,
    ) -> Result<TableDetail, String> {
        let mut conn = connect_and_auth(p)?;
        let (_, topics) = fetch_metadata(&mut conn)?;
        let part_count = topics
            .iter()
            .find(|t| t.name == table)
            .map(|t| t.partitions.len() as i64)
            .unwrap_or(0);

        let columns = [
            ("partition", "int32"),
            ("offset", "int64"),
            ("key", "string"),
            ("value", "string"),
        ]
        .into_iter()
        .map(|(name, ty)| ColumnInfo {
            name: name.to_string(),
            data_type: ty.to_string(),
            nullable: true,
            default: None,
            primary_key: name == "offset",
            unique: false,
            foreign_key: None,
        })
        .collect();

        Ok(TableDetail {
            columns,
            indexes: Vec::new(),
            row_estimate: part_count,
            size: format!("{part_count} partition(s)"),
        })
    }

    fn run_query(&self, p: &Profile, sql: &str) -> Result<QueryResult, String> {
        let mut conn = connect_and_auth(p)?;
        let trimmed = sql.trim().trim_end_matches(';').trim();
        let upper = trimmed.to_uppercase();

        if upper == "BROKERS" {
            let (brokers, _) = fetch_metadata(&mut conn)?;
            return Ok(QueryResult {
                columns: vec![
                    Column { name: "node_id".into(), type_name: "int32".into() },
                    Column { name: "host".into(), type_name: "string".into() },
                    Column { name: "port".into(), type_name: "int32".into() },
                ],
                rows: brokers
                    .into_iter()
                    .map(|b| vec![json!(b.node_id), json!(b.host), json!(b.port)])
                    .collect(),
                tag: None,
            });
        }

        if upper == "TOPICS" || upper.is_empty() {
            let (_, topics) = fetch_metadata(&mut conn)?;
            return Ok(QueryResult {
                columns: vec![
                    Column { name: "topic".into(), type_name: "string".into() },
                    Column { name: "partitions".into(), type_name: "int32".into() },
                ],
                rows: topics
                    .into_iter()
                    .map(|t| vec![json!(t.name), json!(t.partitions.len())])
                    .collect(),
                tag: None,
            });
        }

        let (topic, limit) = parse_consume_target(trimmed)?;
        let records = fetch_topic_records(&mut conn, &topic, limit)?;
        let rows = records
            .into_iter()
            .map(|(part, offset, key, val)| {
                vec![json!(part), json!(offset), json!(key), json!(val)]
            })
            .collect();
        Ok(QueryResult {
            columns: vec![
                Column { name: "partition".into(), type_name: "int32".into() },
                Column { name: "offset".into(), type_name: "int64".into() },
                Column { name: "key".into(), type_name: "string".into() },
                Column { name: "value".into(), type_name: "string".into() },
            ],
            rows,
            tag: None,
        })
    }
}

fn parse_consume_target(input: &str) -> Result<(String, usize), String> {
    let upper = input.to_uppercase();
    let mut limit = 100usize;
    let body = if upper.starts_with("CONSUME ") {
        input[8..].trim()
    } else if upper.starts_with("SELECT ") {
        let from_idx = upper
            .find(" FROM ")
            .ok_or_else(|| "Expected CONSUME <topic> or SELECT * FROM <topic>".to_string())?;
        input[from_idx + 6..].trim()
    } else {
        input
    };
    let mut parts = body.split_whitespace();
    let raw_topic = parts
        .next()
        .ok_or_else(|| "Missing Kafka topic name".to_string())?;
    let topic = raw_topic
        .split('.')
        .last()
        .unwrap_or(raw_topic)
        .trim_matches(|c| c == '"' || c == '`' || c == '\'')
        .to_string();
    while let Some(tok) = parts.next() {
        if tok.eq_ignore_ascii_case("LIMIT") {
            if let Some(n) = parts.next().and_then(|s| s.parse::<usize>().ok()) {
                limit = n;
            }
        }
    }
    Ok((topic, limit))
}

fn connect_and_auth(p: &Profile) -> Result<TcpShim, String> {
    let mut conn = TcpShim::connect(&p.host, p.port, p.tls).map_err(|e| e.to_string())?;
    // Probe ApiVersions v0 to verify broker framing.
    let req = encode_request(API_API_VERSIONS, 0, 1, "seshat", &[]);
    let _ = send_and_recv(&mut conn, &req)?;

    if p.auth != AuthMode::None && !p.user.trim().is_empty() {
        sasl_plain_auth(&mut conn, &p.user, &p.password)?;
    }
    Ok(conn)
}

fn sasl_plain_auth(conn: &mut TcpShim, user: &str, password: &str) -> Result<(), String> {
    let mut hs_body = Vec::new();
    write_str(&mut hs_body, "PLAIN");
    let hs_req = encode_request(API_SASL_HANDSHAKE, 1, 2, "seshat", &hs_body);
    let hs_resp = send_and_recv(conn, &hs_req)?;
    if hs_resp.len() >= 2 {
        let err_code = i16::from_be_bytes([hs_resp[0], hs_resp[1]]);
        if err_code != 0 {
            return Err(format!("Kafka SASL handshake failed (code {err_code})"));
        }
    }

    let mut auth_bytes = Vec::new();
    auth_bytes.push(0);
    auth_bytes.extend_from_slice(user.as_bytes());
    auth_bytes.push(0);
    auth_bytes.extend_from_slice(password.as_bytes());

    let mut body = Vec::new();
    body.extend_from_slice(&(auth_bytes.len() as i32).to_be_bytes());
    body.extend_from_slice(&auth_bytes);
    let req = encode_request(API_SASL_AUTHENTICATE, 0, 3, "seshat", &body);
    let resp = send_and_recv(conn, &req)?;
    if resp.len() >= 2 {
        let err_code = i16::from_be_bytes([resp[0], resp[1]]);
        if err_code != 0 {
            return Err(format!("Kafka SASL authentication failed (code {err_code})"));
        }
    }
    Ok(())
}

fn fetch_metadata(conn: &mut TcpShim) -> Result<(Vec<BrokerInfo>, Vec<TopicInfo>), String> {
    // Metadata v0: empty topic array (length 0) requests all topics.
    let body = 0i32.to_be_bytes();
    let req = encode_request(API_METADATA, 0, 4, "seshat", &body);
    let resp = send_and_recv(conn, &req)?;
    decode_metadata_v0(&resp)
}

fn fetch_topic_records(
    conn: &mut TcpShim,
    topic: &str,
    limit: usize,
) -> Result<Vec<(i32, i64, Option<String>, String)>, String> {
    // ListOffsets v0 for partition 0 earliest (-2)
    let mut lo_body = Vec::new();
    lo_body.extend_from_slice(&(-1i32).to_be_bytes()); // replica_id
    lo_body.extend_from_slice(&1i32.to_be_bytes()); // 1 topic
    write_str(&mut lo_body, topic);
    lo_body.extend_from_slice(&1i32.to_be_bytes()); // 1 partition
    lo_body.extend_from_slice(&0i32.to_be_bytes()); // partition 0
    lo_body.extend_from_slice(&(-2i64).to_be_bytes()); // earliest offset
    lo_body.extend_from_slice(&1i32.to_be_bytes()); // max_num_offsets

    let lo_req = encode_request(API_LIST_OFFSETS, 0, 5, "seshat", &lo_body);
    let lo_resp = send_and_recv(conn, &lo_req)?;
    let start_offset = decode_first_offset_v0(&lo_resp).unwrap_or(0);

    // Fetch v0 for partition 0 starting at start_offset
    let mut f_body = Vec::new();
    f_body.extend_from_slice(&(-1i32).to_be_bytes()); // replica_id
    f_body.extend_from_slice(&100i32.to_be_bytes()); // max_wait_ms
    f_body.extend_from_slice(&1i32.to_be_bytes()); // min_bytes
    f_body.extend_from_slice(&1i32.to_be_bytes()); // 1 topic
    write_str(&mut f_body, topic);
    f_body.extend_from_slice(&1i32.to_be_bytes()); // 1 partition
    f_body.extend_from_slice(&0i32.to_be_bytes()); // partition 0
    f_body.extend_from_slice(&start_offset.to_be_bytes());
    f_body.extend_from_slice(&(1024 * 1024i32).to_be_bytes()); // max_bytes

    let f_req = encode_request(API_FETCH, 0, 6, "seshat", &f_body);
    let f_resp = send_and_recv(conn, &f_req)?;
    decode_fetch_v0(&f_resp, limit)
}

/// Encode a Kafka protocol request frame (`length + header + body`).
pub fn encode_request(
    api_key: i16,
    api_version: i16,
    correlation_id: i32,
    client_id: &str,
    body: &[u8],
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(10 + client_id.len() + body.len());
    payload.extend_from_slice(&api_key.to_be_bytes());
    payload.extend_from_slice(&api_version.to_be_bytes());
    payload.extend_from_slice(&correlation_id.to_be_bytes());
    write_str(&mut payload, client_id);
    payload.extend_from_slice(body);

    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as i32).to_be_bytes());
    frame.extend_from_slice(&payload);
    frame
}

fn send_and_recv(conn: &mut TcpShim, frame: &[u8]) -> Result<Vec<u8>, String> {
    conn.write_all(frame).map_err(|e| e.to_string())?;
    conn.flush().map_err(|e| e.to_string())?;

    let mut len_buf = [0u8; 4];
    conn.read_exact(&mut len_buf).map_err(|e| e.to_string())?;
    let total = i32::from_be_bytes(len_buf) as usize;
    if total < 4 || total > 64 * 1024 * 1024 {
        return Err(format!("Invalid Kafka response length: {total}"));
    }
    let mut payload = vec![0u8; total];
    conn.read_exact(&mut payload).map_err(|e| e.to_string())?;
    // First 4 bytes are correlation_id; return the response body after it.
    Ok(payload[4..].to_vec())
}

fn write_str(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as i16).to_be_bytes());
    buf.extend_from_slice(s.as_bytes());
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn i16(&mut self) -> Result<i16, String> {
        if self.pos + 2 > self.data.len() {
            return Err("Unexpected EOF reading i16".into());
        }
        let v = i16::from_be_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }
    fn i32(&mut self) -> Result<i32, String> {
        if self.pos + 4 > self.data.len() {
            return Err("Unexpected EOF reading i32".into());
        }
        let v = i32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }
    fn i64(&mut self) -> Result<i64, String> {
        if self.pos + 8 > self.data.len() {
            return Err("Unexpected EOF reading i64".into());
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&self.data[self.pos..self.pos + 8]);
        self.pos += 8;
        Ok(i64::from_be_bytes(b))
    }
    fn string(&mut self) -> Result<String, String> {
        let len = self.i16()?;
        if len < 0 {
            return Ok(String::new());
        }
        let len = len as usize;
        if self.pos + len > self.data.len() {
            return Err("Unexpected EOF reading Kafka string".into());
        }
        let s = String::from_utf8_lossy(&self.data[self.pos..self.pos + len]).to_string();
        self.pos += len;
        Ok(s)
    }
    fn bytes_opt(&mut self) -> Result<Option<Vec<u8>>, String> {
        let len = self.i32()?;
        if len < 0 {
            return Ok(None);
        }
        let len = len as usize;
        if self.pos + len > self.data.len() {
            return Err("Unexpected EOF reading Kafka bytes".into());
        }
        let b = self.data[self.pos..self.pos + len].to_vec();
        self.pos += len;
        Ok(Some(b))
    }
}

/// Decode a Kafka `Metadata` v0 response body.
pub fn decode_metadata_v0(data: &[u8]) -> Result<(Vec<BrokerInfo>, Vec<TopicInfo>), String> {
    let mut c = Cursor::new(data);
    let broker_count = c.i32()?.max(0) as usize;
    let mut brokers = Vec::with_capacity(broker_count);
    for _ in 0..broker_count {
        brokers.push(BrokerInfo {
            node_id: c.i32()?,
            host: c.string()?,
            port: c.i32()?,
        });
    }

    let topic_count = c.i32()?.max(0) as usize;
    let mut topics = Vec::with_capacity(topic_count);
    for _ in 0..topic_count {
        let _err = c.i16()?;
        let name = c.string()?;
        let part_count = c.i32()?.max(0) as usize;
        let mut partitions = Vec::with_capacity(part_count);
        for _ in 0..part_count {
            let _perr = c.i16()?;
            let id = c.i32()?;
            let leader = c.i32()?;
            let rep_count = c.i32()?.max(0) as usize;
            let mut replicas = Vec::with_capacity(rep_count);
            for _ in 0..rep_count {
                replicas.push(c.i32()?);
            }
            let isr_count = c.i32()?.max(0) as usize;
            let mut isr = Vec::with_capacity(isr_count);
            for _ in 0..isr_count {
                isr.push(c.i32()?);
            }
            partitions.push(PartitionInfo {
                id,
                leader,
                replicas,
                isr,
            });
        }
        topics.push(TopicInfo { name, partitions });
    }
    Ok((brokers, topics))
}

fn decode_first_offset_v0(data: &[u8]) -> Result<i64, String> {
    let mut c = Cursor::new(data);
    let topics = c.i32()?;
    if topics <= 0 {
        return Ok(0);
    }
    let _topic = c.string()?;
    let parts = c.i32()?;
    if parts <= 0 {
        return Ok(0);
    }
    let _part = c.i32()?;
    let _err = c.i16()?;
    let offsets = c.i32()?;
    if offsets > 0 {
        return c.i64();
    }
    Ok(0)
}

fn decode_fetch_v0(
    data: &[u8],
    limit: usize,
) -> Result<Vec<(i32, i64, Option<String>, String)>, String> {
    let mut c = Cursor::new(data);
    let topics = c.i32()?.max(0) as usize;
    let mut out = Vec::new();
    for _ in 0..topics {
        let _topic = c.string()?;
        let parts = c.i32()?.max(0) as usize;
        for _ in 0..parts {
            let partition = c.i32()?;
            let _err = c.i16()?;
            let _hwm = c.i64()?;
            if let Some(msg_set) = c.bytes_opt()? {
                let mut mc = Cursor::new(&msg_set);
                while mc.pos + 12 <= mc.data.len() && out.len() < limit {
                    let offset = mc.i64()?;
                    let msg_size = mc.i32()?.max(0) as usize;
                    if mc.pos + msg_size > mc.data.len() {
                        break;
                    }
                    let msg_slice = &mc.data[mc.pos..mc.pos + msg_size];
                    mc.pos += msg_size;
                    // Legacy MessageSet v0/v1: crc(4) + magic(1) + attrs(1) [+ ts(8) if magic==1] + key + val
                    if msg_slice.len() >= 14 {
                        let magic = msg_slice[4];
                        let mut vc = Cursor::new(&msg_slice[6..]);
                        if magic == 1 {
                            let _ = vc.i64();
                        }
                        let key = vc
                            .bytes_opt()
                            .ok()
                            .flatten()
                            .map(|b| String::from_utf8_lossy(&b).to_string());
                        let val = vc
                            .bytes_opt()
                            .ok()
                            .flatten()
                            .map(|b| String::from_utf8_lossy(&b).to_string())
                            .unwrap_or_default();
                        out.push((partition, offset, key, val));
                    }
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_kafka_request_frame() {
        let frame = encode_request(API_API_VERSIONS, 0, 7, "seshat", &[]);
        // 4-byte length + 2 api_key + 2 api_ver + 4 corr_id + 2 str_len + 6 "seshat" = 20
        assert_eq!(frame.len(), 20);
        let len = i32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]);
        assert_eq!(len, 16);
        let corr = i32::from_be_bytes([frame[8], frame[9], frame[10], frame[11]]);
        assert_eq!(corr, 7);
    }

    #[test]
    fn decodes_kafka_metadata_v0() {
        let mut body = Vec::new();
        // 1 broker
        body.extend_from_slice(&1i32.to_be_bytes());
        body.extend_from_slice(&0i32.to_be_bytes());
        write_str(&mut body, "localhost");
        body.extend_from_slice(&9092i32.to_be_bytes());
        // 1 topic
        body.extend_from_slice(&1i32.to_be_bytes());
        body.extend_from_slice(&0i16.to_be_bytes());
        write_str(&mut body, "events");
        body.extend_from_slice(&1i32.to_be_bytes());
        body.extend_from_slice(&0i16.to_be_bytes());
        body.extend_from_slice(&0i32.to_be_bytes()); // part 0
        body.extend_from_slice(&0i32.to_be_bytes()); // leader 0
        body.extend_from_slice(&0i32.to_be_bytes()); // 0 replicas
        body.extend_from_slice(&0i32.to_be_bytes()); // 0 isr

        let (brokers, topics) = decode_metadata_v0(&body).unwrap();
        assert_eq!(brokers.len(), 1);
        assert_eq!(brokers[0].host, "localhost");
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].name, "events");
        assert_eq!(topics[0].partitions.len(), 1);
    }
}
