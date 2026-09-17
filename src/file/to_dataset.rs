//! Convert an open file into a tabular dataset for the data bus (#113) —
//! so Thoth core (the file viewer) and every file-loader plugin (csv-loader,
//! …) are producers too.
//!
//! The conversion reads straight from a tab's live [`FileLoader`] as Arrow
//! and formats the batches into strings. Because every format — native or
//! plugin-loaded — reaches the loader through DuckDB, this one path covers
//! JSON, NDJSON, CSV, Parquet, databases and plugin formats alike.
//!
//! [`records_to_dataset`] remains for callers that already hold JSON records
//! (plugin data sources, chart studio) and never touch a file loader.

use duckdb::arrow::array::RecordBatch;
use duckdb::arrow::datatypes::DataType;
use duckdb::arrow::util::display::{ArrayFormatter, FormatOptions};
use serde_json::Value;

use crate::file::loaders::{FileLoader, batch_rows};

/// Rows read from the file (bounds the crossing for large files).
const CAP: usize = 5000;

/// `(columns, rows)` where each column is `(name, sql-ish type hint)` and each
/// row is a list of string cells.
pub type DatasetTable = (Vec<(String, String)>, Vec<Vec<String>>);

/// Read up to [`CAP`] rows from a live loader and map them to a
/// [`DatasetTable`]. `None` if the loader yields no readable rows.
pub fn loader_to_dataset(loader: &dyn FileLoader) -> Option<DatasetTable> {
    let batches = loader.fetch(Vec::new(), None, Some(CAP)).ok()?;
    batches_to_dataset(&batches)
}

/// Map Arrow batches to a [`DatasetTable`]. `None` if there are no rows.
pub fn batches_to_dataset(batches: &[RecordBatch]) -> Option<DatasetTable> {
    let first = batches.iter().find(|b| b.num_rows() > 0)?;

    let cols: Vec<(String, String)> = first
        .schema()
        .fields()
        .iter()
        .map(|f| (f.name().to_string(), type_hint_arrow(f.data_type())))
        .collect();
    if cols.is_empty() {
        return None;
    }

    let opts = FormatOptions::default().with_null("");
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(batch_rows(batches).min(CAP));

    for batch in batches.iter().filter(|b| b.num_rows() > 0) {
        // One formatter per column, reused across every row in the batch.
        let formatters: Vec<ArrayFormatter> = match batch
            .columns()
            .iter()
            .map(|col| ArrayFormatter::try_new(col.as_ref(), &opts))
            .collect::<Result<_, _>>()
        {
            Ok(f) => f,
            Err(_) => continue,
        };
        for row in 0..batch.num_rows() {
            rows.push(
                formatters
                    .iter()
                    .map(|f| f.value(row).to_string())
                    .collect(),
            );
            if rows.len() >= CAP {
                return Some((cols, rows));
            }
        }
    }

    if rows.is_empty() {
        None
    } else {
        Some((cols, rows))
    }
}

/// Map already-loaded JSON records to a [`DatasetTable`]. `None` if empty.
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

/// Map an Arrow type to the same sql-ish hints [`type_hint`] produces, so a
/// dataset looks the same whether it arrived as Arrow or as JSON records.
fn type_hint_arrow(dt: &DataType) -> String {
    use DataType::*;
    match dt {
        Int8 | Int16 | Int32 | Int64 | UInt8 | UInt16 | UInt32 | UInt64 => "integer",
        Float16 | Float32 | Float64 | Decimal128(_, _) | Decimal256(_, _) => "float",
        Boolean => "boolean",
        Utf8 | LargeUtf8 | Utf8View => "text",
        Date32 | Date64 => "date",
        Timestamp(_, _) => "timestamp",
        Time32(_) | Time64(_) => "time",
        _ => "",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::loaders::duck_db::DuckdbConnection;
    use serde_json::json;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn ndjson(lines: &str) -> NamedTempFile {
        let mut tmp = tempfile::Builder::new()
            .suffix(".ndjson")
            .tempfile()
            .unwrap();
        tmp.write_all(lines.as_bytes()).unwrap();
        tmp.flush().unwrap();
        tmp
    }

    #[test]
    fn empty_records_none() {
        assert!(records_to_dataset(&[]).is_none());
    }

    #[test]
    fn loader_to_dataset_reads_a_live_loader() {
        let file = ndjson("{\"a\":1,\"b\":\"x\"}\n{\"a\":2,\"b\":\"y\"}\n");
        let db = DuckdbConnection::open_path(file.path()).unwrap();

        let (cols, rows) = loader_to_dataset(&db).unwrap();
        assert_eq!(
            cols.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(cols[0].1, "integer");
        assert_eq!(cols[1].1, "text");
        assert_eq!(rows, vec![vec!["1", "x"], vec!["2", "y"]]);
    }

    #[test]
    fn loader_to_dataset_reads_past_one_arrow_batch() {
        // DuckDB emits ~2048-row batches, so this spans several.
        let n = 5000;
        let lines: String = (0..n).map(|i| format!("{{\"n\":{i}}}\n")).collect();
        let file = ndjson(&lines);
        let db = DuckdbConnection::open_path(file.path()).unwrap();

        let (cols, rows) = loader_to_dataset(&db).unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(rows.len(), CAP.min(n));
        // Order is preserved across batch seams.
        assert_eq!(rows[0][0], "0");
        assert_eq!(rows[3000][0], "3000");
    }

    #[test]
    fn loader_to_dataset_caps_large_files() {
        let lines: String = (0..CAP + 500).map(|i| format!("{{\"n\":{i}}}\n")).collect();
        let file = ndjson(&lines);
        let db = DuckdbConnection::open_path(file.path()).unwrap();

        let (_, rows) = loader_to_dataset(&db).unwrap();
        assert_eq!(rows.len(), CAP);
    }

    #[test]
    fn nulls_become_empty_cells() {
        let file = ndjson("{\"a\":1,\"b\":\"x\"}\n{\"a\":null,\"b\":\"y\"}\n");
        let db = DuckdbConnection::open_path(file.path()).unwrap();

        let (_, rows) = loader_to_dataset(&db).unwrap();
        assert_eq!(rows[1][0], "");
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
