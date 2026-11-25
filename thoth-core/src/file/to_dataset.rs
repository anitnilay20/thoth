//! Convert an open file into a tabular dataset for the data bus (#113) —
//! so Thoth core (the file viewer) and every file-loader plugin are producers
//! too.
//!
//! The conversion reads straight from a tab's live [`FileLoader`] as Arrow
//! and formats the batches into strings. Because every format — native or
//! plugin-loaded — reaches the loader through DuckDB, this one path covers
//! JSON, NDJSON, CSV, Parquet, databases and plugin formats alike.
//!
//! [`records_to_dataset`] remains for callers that already hold JSON records
//! (plugin data sources, chart studio) and never touch a file loader.

use duckdb::arrow::array::{Array, RecordBatch};
use duckdb::arrow::datatypes::DataType;
use duckdb::arrow::util::display::{ArrayFormatter, FormatOptions, array_value_to_string};
use serde_json::Value;

use crate::error::{Result as ThothResult, ThothError};
use crate::file::loaders::FileLoader;

/// Rows read from the file (bounds the crossing for large files).
pub const CAP: usize = 5000;

/// `(columns, rows)` where each column is `(name, sql-ish type hint)` and each
/// row is a list of string cells.
pub type DatasetTable = (Vec<(String, String)>, Vec<Vec<String>>);

/// Read up to [`CAP`] rows from a live loader and map them to a
/// [`DatasetTable`]. `None` if the loader yields no readable rows.
pub fn loader_to_dataset(loader: &dyn FileLoader) -> Option<DatasetTable> {
    let batches = loader.fetch(Vec::new(), None, Some(CAP)).ok()?;
    batches_to_dataset(&batches)
}

/// Map Arrow batches to string rows without [`CAP`].
///
/// Used when a typed, batch-backed sheet is mutated through the string
/// `dataset-bus.update`/`append` ABI. If a column cannot be formatted, the
/// whole conversion fails (`None`) rather than returning partial rows, so the
/// caller can keep the original Arrow source. Display callers should prefer
/// [`batches_to_dataset`], which is bounded.
pub fn batches_to_rows(batches: &[RecordBatch]) -> Option<Vec<Vec<String>>> {
    collect_rows(batches, usize::MAX)
}

fn collect_rows(batches: &[RecordBatch], cap: usize) -> Option<Vec<Vec<String>>> {
    let opts = FormatOptions::default().with_null("");
    let mut rows = Vec::new();
    for batch in batches.iter().filter(|b| b.num_rows() > 0) {
        let formatters: Option<Vec<ArrayFormatter>> = batch
            .columns()
            .iter()
            .map(|col| ArrayFormatter::try_new(col.as_ref(), &opts))
            .collect::<Result<_, _>>()
            .ok();
        for row in 0..batch.num_rows() {
            let cells = match &formatters {
                Some(formatters) => formatters
                    .iter()
                    .map(|formatter| formatter.value(row).try_to_string().ok())
                    .collect::<Option<Vec<_>>>()?,
                // A formatter can reject an unusual Arrow type. Fall back to
                // the generic path for every cell; if that fails too, fail the
                // whole conversion rather than dropping the batch.
                None => {
                    let mut cells = Vec::with_capacity(batch.num_columns());
                    for column in batch.columns() {
                        cells.push(array_value_to_string(column.as_ref(), row).ok()?);
                    }
                    cells
                }
            };
            rows.push(cells);
            if rows.len() >= cap {
                return Some(rows);
            }
        }
    }
    Some(rows)
}

/// Fallible Arrow-to-dataset conversion.
///
/// `Ok(None)` means the batches hold no rows, which is a valid empty window;
/// `Err` means a value could not be formatted. Display code must not collapse
/// the two into an empty page.
pub fn try_batches_to_dataset(batches: &[RecordBatch]) -> ThothResult<Option<DatasetTable>> {
    let Some(first) = batches.iter().find(|b| b.num_rows() > 0) else {
        return Ok(None);
    };
    let cols: Vec<(String, String)> = first
        .schema()
        .fields()
        .iter()
        .map(|f| (f.name().to_string(), type_hint_arrow(f.data_type())))
        .collect();
    if cols.is_empty() {
        return Ok(None);
    }
    let rows = collect_rows(batches, CAP).ok_or_else(|| ThothError::Unknown {
        message: "failed to format Arrow values as text".to_string(),
    })?;
    Ok(if rows.is_empty() {
        None
    } else {
        Some((cols, rows))
    })
}

/// Map Arrow batches to a [`DatasetTable`]. `None` if there are no rows or a
/// column cannot be formatted (never a partial table). Callers that must tell
/// an empty window from a formatting failure use [`try_batches_to_dataset`].
pub fn batches_to_dataset(batches: &[RecordBatch]) -> Option<DatasetTable> {
    try_batches_to_dataset(batches).ok().flatten()
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
        // A nested value is JSON to everything downstream: the grid draws it as
        // a chip rather than a wall of braces, and the tree walks into it.
        // Without this it reads as text and the formatted JSON fills the cell.
        Struct(_)
        | List(_)
        | LargeList(_)
        | ListView(_)
        | LargeListView(_)
        | FixedSizeList(_, _)
        | Map(_, _)
        | Union(_, _) => "json",
        _ => "",
    }
    .to_string()
}

/// Which cells of the first [`CAP`] rows were NULL, in the same shape as
/// [`batches_to_dataset`]'s rows.
///
/// Arrow formats a NULL as the empty string, which is also a perfectly good
/// value — so the mask is the only thing that can tell a field the record does
/// not carry from one that carries `""`. Kept separate from the rows so export
/// and the data bus, which want the empty string, are unaffected.
pub fn batch_nulls(batches: &[RecordBatch]) -> Vec<Vec<bool>> {
    let mut mask: Vec<Vec<bool>> = Vec::new();
    for batch in batches.iter().filter(|b| b.num_rows() > 0) {
        for row in 0..batch.num_rows() {
            mask.push(batch.columns().iter().map(|c| c.is_null(row)).collect());
            if mask.len() >= CAP {
                return mask;
            }
        }
    }
    mask
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
    fn the_null_mask_separates_an_absent_field_from_an_empty_one() {
        // Row 0's `b` is the empty string, row 1's is absent. Both format as
        // "", so only the mask can tell the grid which is which.
        let file = ndjson("{\"a\":1,\"b\":\"\"}\n{\"a\":2}\n");
        let db = DuckdbConnection::open_path(file.path()).unwrap();
        let batches = db.fetch(Vec::new(), None, Some(CAP)).unwrap();

        let (cols, rows) = batches_to_dataset(&batches).unwrap();
        let b = cols.iter().position(|(n, _)| n == "b").unwrap();
        assert_eq!(rows[0][b], "");
        assert_eq!(rows[1][b], "");

        let nulls = batch_nulls(&batches);
        assert!(!nulls[0][b], "an empty string is a value");
        assert!(nulls[1][b], "a field the record does not carry is absent");
    }

    #[test]
    fn the_null_mask_matches_the_rows_it_describes() {
        // Spanning several Arrow batches, so a per-batch mask that forgot to
        // accumulate would be caught.
        let lines: String = (0..5000).map(|i| format!("{{\"n\":{i}}}\n")).collect();
        let file = ndjson(&lines);
        let db = DuckdbConnection::open_path(file.path()).unwrap();
        let batches = db.fetch(Vec::new(), None, Some(CAP)).unwrap();

        let (cols, rows) = batches_to_dataset(&batches).unwrap();
        let nulls = batch_nulls(&batches);
        assert_eq!(nulls.len(), rows.len());
        assert!(nulls.iter().all(|r| r.len() == cols.len()));
    }

    #[test]
    fn a_nested_value_is_typed_as_json() {
        // Without this the grid reads a struct as text and renders the whole
        // formatted object into one 22px cell.
        let file = ndjson("{\"a\":1,\"t\":{\"id\":\"x\",\"ok\":true}}\n");
        let db = DuckdbConnection::open_path(file.path()).unwrap();

        let (cols, _) = loader_to_dataset(&db).unwrap();
        let t = cols.iter().find(|(n, _)| n == "t").expect("column present");
        assert_eq!(t.1, "json");
    }

    #[test]
    fn object_rows_union_keys() {
        // A file-loader plugin's shape: each record is an object (one row).
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

    #[test]
    fn an_out_of_range_date_fails_the_conversion() {
        use duckdb::arrow::array::Date32Array;
        use duckdb::arrow::datatypes::{DataType, Field, Schema};
        use std::sync::Arc;

        let schema = Arc::new(Schema::new(vec![Field::new(
            "day",
            DataType::Date32,
            false,
        )]));
        let batch = RecordBatch::try_new(schema, vec![Arc::new(Date32Array::from(vec![i32::MAX]))])
            .unwrap();

        assert!(
            batches_to_rows(&[batch]).is_none(),
            "a per-value formatting error must fail the conversion, not emit text"
        );
    }

    #[test]
    fn a_named_timezone_never_becomes_display_error_text() {
        use duckdb::arrow::array::TimestampMicrosecondArray;
        use duckdb::arrow::datatypes::{DataType, Field, Schema, TimeUnit};
        use std::sync::Arc;

        let schema = Arc::new(Schema::new(vec![Field::new(
            "at",
            DataType::Timestamp(TimeUnit::Microsecond, Some("America/New_York".into())),
            false,
        )]));
        let array =
            TimestampMicrosecondArray::from(vec![1_000_000]).with_timezone("America/New_York");
        let batch = RecordBatch::try_new(schema, vec![Arc::new(array)]).unwrap();

        let rows = batches_to_rows(&[batch]);
        assert!(
            rows.is_none(),
            "an unformattable value must fail the conversion, not emit error text"
        );
    }
}
