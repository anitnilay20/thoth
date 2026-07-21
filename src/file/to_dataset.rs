//! Convert an open JSON/NDJSON file into a tabular dataset for the data bus
//! (#113) — so Thoth core (the file viewer) is a producer too.
//!
//! Best-effort v1 mapping (capped so a huge file never fully crosses):
//! - array/NDJSON of objects → columns = union of keys (first-seen), one row
//!   per record;
//! - a single JSON object → a two-column `key` / `value` table;
//! - scalars/arrays → a single `value` column.

use std::path::Path;

use serde_json::Value;

use crate::file::loaders::{FileType, load_file_auto};

/// Rows read from the file (bounds the crossing for large files).
const CAP: usize = 5000;

/// `(columns, rows)` where each column is `(name, sql-ish type hint)` and each
/// row is a list of string cells.
pub type DatasetTable = (Vec<(String, String)>, Vec<Vec<String>>);

/// Read the first rows of `path` as a [`DatasetTable`]. `None` if it can't be
/// read as tabular.
pub fn file_to_dataset(path: &Path) -> Option<DatasetTable> {
    let (_detected, mut loader): (_, FileType) = load_file_auto(path).ok()?;
    let n = loader.len().min(CAP);
    let mut records: Vec<Value> = Vec::with_capacity(n);
    for i in 0..n {
        if let Ok(v) = loader.get(i) {
            records.push(v);
        }
    }
    if records.is_empty() {
        return None;
    }

    // A single JSON object → key / value table.
    if records.len() == 1
        && let Value::Object(map) = &records[0]
    {
        let cols = vec![
            ("key".to_string(), "text".to_string()),
            ("value".to_string(), "text".to_string()),
        ];
        let rows = map
            .iter()
            .map(|(k, v)| vec![k.clone(), cell_string(v)])
            .collect();
        return Some((cols, rows));
    }

    // Object records → tabular (union of keys, first-seen order).
    if records.iter().any(Value::is_object) {
        let mut keys: Vec<String> = Vec::new();
        for r in &records {
            if let Value::Object(m) = r {
                for k in m.keys() {
                    if !keys.iter().any(|e| e == k) {
                        keys.push(k.clone());
                    }
                }
            }
        }
        let cols: Vec<(String, String)> = keys
            .iter()
            .map(|k| {
                let hint = records
                    .iter()
                    .find_map(|r| r.get(k))
                    .map(type_hint)
                    .unwrap_or_default();
                (k.clone(), hint)
            })
            .collect();
        let rows = records
            .iter()
            .map(|r| {
                keys.iter()
                    .map(|k| r.get(k).map(cell_string).unwrap_or_default())
                    .collect()
            })
            .collect();
        return Some((cols, rows));
    }

    // Scalars / arrays → a single `value` column.
    let cols = vec![("value".to_string(), type_hint(&records[0]))];
    let rows = records.iter().map(|v| vec![cell_string(v)]).collect();
    Some((cols, rows))
}

fn cell_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        // numbers, bools → literal; arrays/objects → compact JSON
        other => other.to_string(),
    }
}

fn type_hint(v: &Value) -> String {
    match v {
        Value::Number(n) => if n.is_i64() || n.is_u64() {
            "integer"
        } else {
            "float"
        }
        .to_string(),
        Value::Bool(_) => "boolean".to_string(),
        Value::String(_) => "text".to_string(),
        _ => String::new(),
    }
}
