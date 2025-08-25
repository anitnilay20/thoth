use anyhow::{Context, Result, anyhow};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedFileType {
    Ndjson,
    JsonArray,
    JsonObject,
}

pub fn sniff_file_type(path: &Path) -> Result<DetectedFileType> {
    let file = File::open(path).with_context(|| "open for sniffing")?;
    let mut reader = BufReader::new(file);

    // Read a small prefix to find the first non-ws char
    let mut prefix = [0u8; 8192];
    let n = reader.read(&mut prefix)?;
    if n == 0 {
        return Err(anyhow!("empty file"));
    }
    let bytes = &prefix[..n];

    // Skip UTF-8 BOM if present
    let mut i = 0usize;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        i = 3;
    }
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\n' | b'\r' | b'\t') {
        i += 1;
    }
    let first = *bytes.get(i).ok_or_else(|| anyhow!("no content"))?;

    if first == b'[' {
        return Ok(DetectedFileType::JsonArray);
    }
    if first != b'{' {
        // Strictly speaking NDJSON lines can start with [ as well, but common case is '{'
        // If it's not '[' or '{', treat it as NDJSON only if first two lines parse as JSON.
        return ndjson_if_two_lines_parse(path);
    }

    // Starts with '{' – could be Object or NDJSON. Check first two non-empty lines.
    ndjson_if_two_lines_parse(path).or_else(|_| Ok(DetectedFileType::JsonObject))
}

fn ndjson_if_two_lines_parse(path: &Path) -> Result<DetectedFileType> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let mut valid = 0usize;
    let mut buf = String::new();
    // Look only at the first few non-empty lines
    for _ in 0..8 {
        buf.clear();
        if reader.read_line(&mut buf)? == 0 {
            break;
        }
        let line = buf.trim();
        if line.is_empty() {
            continue;
        }

        if serde_json::from_str::<serde_json::Value>(line).is_ok() {
            valid += 1;
            if valid >= 2 {
                return Ok(DetectedFileType::Ndjson);
            }
        } else {
            // First non-empty line didn't parse → not NDJSON
            break;
        }
    }
    Err(anyhow!("not NDJSON by line sniff"))
}
