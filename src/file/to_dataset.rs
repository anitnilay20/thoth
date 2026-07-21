//! Convert an open file tab into a tabular dataset for the data bus (#113) —
//! so Thoth core (the file viewer) and every file-loader plugin (csv-loader,
//! …) are producers too.
//!
//! The conversion reads records straight from a tab's *live* [`FileType`]
//! loader via [`FileType::get`], which yields a [`Value`] regardless of the
//! backing format (JSON, NDJSON, or a WASM file-loader plugin like csv-loader).
//! That's what makes every file-loader plugin a producer "by default".
//!
//! Best-effort v1 mapping (capped so a huge file never fully crosses):
//! - array/NDJSON of objects → columns = union of keys (first-seen), one row
//!   per record;
//! - a single JSON object → a two-column `key` / `value` table;
//! - scalars/arrays → a single `value` column.

use serde_json::Value;

use crate::file::loaders::FileType;

/// Rows read from the file (bounds the crossing for large files).
const CAP: usize = 5000;

/// Rows fetched per bulk `get_range` call.
const CHUNK: usize = 2000;

/// `(columns, rows)` where each column is `(name, sql-ish type hint)` and each
/// row is a list of string cells.
pub type DatasetTable = (Vec<(String, String)>, Vec<Vec<String>>);

/// Read up to [`CAP`] records from a live loader and map them to a
/// [`DatasetTable`]. Works for any [`FileType`] (JSON/NDJSON and plugin
/// loaders alike). `None` if the loader yields no readable records.
pub fn loader_to_dataset(loader: &mut FileType) -> Option<DatasetTable> {
    let n = loader.len().min(CAP);
    let mut records: Vec<Value> = Vec::with_capacity(n);
    // Read in bulk chunks via `get_range` — a single sequential pass per chunk
    // instead of `n` per-record crossings (O(n²) for stream-parsed formats).
    let mut start = 0;
    while start < n {
        let count = CHUNK.min(n - start);
        match loader.get_range(start, count) {
            Ok(chunk) => {
                if chunk.is_empty() {
                    break; // file ended early
                }
                let got = chunk.len();
                records.extend(chunk);
                start += got;
            }
            Err(_) => break,
        }
    }
    records_to_dataset(&records)
}

/// Map already-loaded records to a [`DatasetTable`]. `None` if empty.
pub fn records_to_dataset(records: &[Value]) -> Option<DatasetTable> {
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
        for r in records {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_records_none() {
        assert!(records_to_dataset(&[]).is_none());
    }

    #[test]
    fn object_rows_union_keys() {
        // The csv-loader shape: each record is an object (one CSV row).
        let recs = vec![
            json!({ "name": "ada", "age": 36 }),
            json!({ "name": "linus", "city": "helsinki" }),
        ];
        let (cols, rows) = records_to_dataset(&recs).unwrap();
        let names: Vec<&str> = cols.iter().map(|(n, _)| n.as_str()).collect();
        // Columns are the union of all record keys.
        let col = |k: &str| names.iter().position(|n| *n == k).expect("column present");
        assert_eq!(names.len(), 3);
        // Missing keys become empty cells; present keys stringify.
        assert_eq!(rows[0][col("name")], "ada");
        assert_eq!(rows[0][col("age")], "36");
        assert_eq!(rows[0][col("city")], "");
        assert_eq!(rows[1][col("name")], "linus");
        assert_eq!(rows[1][col("age")], "");
        assert_eq!(rows[1][col("city")], "helsinki");
    }

    #[test]
    fn single_object_key_value() {
        let recs = vec![json!({ "a": 1, "b": "x" })];
        let (cols, rows) = records_to_dataset(&recs).unwrap();
        assert_eq!(
            cols.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            ["key", "value"]
        );
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn scalars_single_value_column() {
        let recs = vec![json!(1), json!(2), json!(3)];
        let (cols, rows) = records_to_dataset(&recs).unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].0, "value");
        assert_eq!(rows, vec![vec!["1"], vec!["2"], vec!["3"]]);
    }
}
