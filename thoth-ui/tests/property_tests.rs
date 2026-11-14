//! Invariants the file loader must hold for any input, checked against the
//! DuckDB query engine (#147/#148) that now backs every format.

use proptest::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;
use thoth::file::loaders::{DuckdbConnection, FileLoader, RecordSource};

/// A temp file with a real extension — the engine picks its reader from the
/// path, so a suffix-less temp file has no format to detect.
fn temp_file(suffix: &str, write: impl FnOnce(&mut NamedTempFile)) -> NamedTempFile {
    let mut file = tempfile::Builder::new()
        .suffix(suffix)
        .tempfile()
        .expect("temp file");
    write(&mut file);
    file.flush().expect("flush");
    file
}

fn ndjson(num_records: usize) -> NamedTempFile {
    temp_file(".ndjson", |file| {
        for i in 0..num_records {
            writeln!(file, r#"{{"id":{}}}"#, i).unwrap();
        }
    })
}

fn json_array(num_elements: usize) -> NamedTempFile {
    temp_file(".json", |file| {
        write!(file, "[").unwrap();
        for i in 0..num_elements {
            write!(file, r#"{{"id":{}}}"#, i).unwrap();
            if i + 1 < num_elements {
                write!(file, ",").unwrap();
            }
        }
        write!(file, "]").unwrap();
    })
}

proptest! {
    // Property: For any number of NDJSON records, len() should equal the number of records
    #[test]
    fn test_ndjson_len_matches_records(num_records in 1usize..100) {
        let file = ndjson(num_records);
        let loader = DuckdbConnection::open_path(file.path()).unwrap();
        prop_assert_eq!(loader.len().unwrap(), num_records);
    }

    // Property: All valid indices should succeed, out of bounds should fail
    #[test]
    fn test_ndjson_bounds_invariant(num_records in 1usize..50) {
        let file = ndjson(num_records);
        let loader = DuckdbConnection::open_path(file.path()).unwrap();

        // All valid indices should succeed
        for i in 0..num_records {
            prop_assert!(loader.record(i).is_ok(), "index {} should be readable", i);
        }

        // Out of bounds should fail
        prop_assert!(loader.record(num_records).is_err());
        prop_assert!(loader.record(num_records + 100).is_err());
        prop_assert!(loader.get(num_records).is_err());
    }

    // Property: raw_bytes should return valid JSON
    #[test]
    fn test_ndjson_raw_bytes_valid_json(num_records in 1usize..20) {
        let file = temp_file(".ndjson", |file| {
            for i in 0..num_records {
                writeln!(file, "{{\"id\":{},\"value\":\"record_{}\"}}", i, i).unwrap();
            }
        });
        let loader = DuckdbConnection::open_path(file.path()).unwrap();

        // Every raw_bytes should be parseable JSON
        for i in 0..num_records {
            let raw = loader.raw_bytes(i).unwrap();
            let parsed: Result<serde_json::Value, _> = serde_json::from_slice(&raw);
            prop_assert!(parsed.is_ok(), "Failed to parse raw bytes at index {}: {:?}", i, String::from_utf8_lossy(&raw));
        }
    }

    // Property: record() results should match raw_bytes when parsed
    #[test]
    fn test_ndjson_record_matches_raw_bytes(num_records in 1usize..20, idx in 0usize..19) {
        let num_records = num_records.max(idx + 1); // Ensure idx is valid
        let file = ndjson(num_records);
        let loader = DuckdbConnection::open_path(file.path()).unwrap();

        let parsed = loader.record(idx).unwrap();
        let raw = loader.raw_bytes(idx).unwrap();
        let from_raw: serde_json::Value = serde_json::from_slice(&raw).unwrap();

        prop_assert_eq!(parsed, from_raw);
    }

    // Property: rows come back in file order, whatever the window boundaries
    #[test]
    fn test_ndjson_preserves_order(num_records in 1usize..60, start in 0usize..30) {
        let file = ndjson(num_records);
        let loader = DuckdbConnection::open_path(file.path()).unwrap();

        let start = start.min(num_records.saturating_sub(1));
        let rows = loader.record_range(start, 10).unwrap();
        for (offset, row) in rows.iter().enumerate() {
            prop_assert_eq!(row["id"].as_u64().unwrap() as usize, start + offset);
        }
    }

    // Property: JSON array with N elements should have len() == N
    #[test]
    fn test_json_array_len_invariant(num_elements in 1usize..50) {
        let file = json_array(num_elements);
        let loader = DuckdbConnection::open_path(file.path()).unwrap();
        prop_assert_eq!(loader.len().unwrap(), num_elements);
    }

    // Property: All indices < len() should be accessible
    #[test]
    fn test_json_array_all_indices_accessible(num_elements in 1usize..30) {
        let file = json_array(num_elements);
        let loader = DuckdbConnection::open_path(file.path()).unwrap();

        for i in 0..num_elements {
            let val = loader.record(i).unwrap();
            prop_assert_eq!(val["id"].as_u64().unwrap() as usize, i);
        }
    }

    // Property: a single top-level object is exactly one row
    #[test]
    fn test_single_value_len_always_one(value in 0i32..1000) {
        let file = temp_file(".json", |file| {
            write!(file, r#"{{"value":{}}}"#, value).unwrap();
        });
        let loader = DuckdbConnection::open_path(file.path()).unwrap();

        prop_assert_eq!(loader.len().unwrap(), 1);
        prop_assert!(!loader.is_empty().unwrap());
    }

    // Property: record(0) always succeeds for one object, record(n>0) always fails
    #[test]
    fn test_single_value_index_invariant(value in 0i32..100, invalid_idx in 1usize..100) {
        let file = temp_file(".json", |file| {
            write!(file, r#"{{"value":{}}}"#, value).unwrap();
        });
        let loader = DuckdbConnection::open_path(file.path()).unwrap();

        // Index 0 should always succeed
        prop_assert!(loader.record(0).is_ok());

        // Any index > 0 should fail
        prop_assert!(loader.record(invalid_idx).is_err());
    }

    // Property: repeated reads return the same value
    #[test]
    fn test_repeated_reads_consistent(value in 0i32..100) {
        let file = temp_file(".json", |file| {
            write!(file, r#"{{"value":{}}}"#, value).unwrap();
        });
        let loader = DuckdbConnection::open_path(file.path()).unwrap();

        let val1 = loader.record(0).unwrap();
        let val2 = loader.record(0).unwrap();
        let val3 = loader.record(0).unwrap();

        prop_assert_eq!(&val1, &val2);
        prop_assert_eq!(&val2, &val3);
        prop_assert_eq!(val1["value"].as_i64().unwrap(), value as i64);
    }

    // Property: is_empty() iff len() == 0
    #[test]
    fn test_is_empty_iff_len_zero(num_records in 1usize..50) {
        let file = json_array(num_records);
        let loader = DuckdbConnection::open_path(file.path()).unwrap();

        prop_assert!(!loader.is_empty().unwrap());
        prop_assert_eq!(loader.is_empty().unwrap(), loader.len().unwrap() == 0);
    }

    // Property: a WHERE predicate never returns more rows than the file holds
    #[test]
    fn test_filter_narrows_the_row_set(num_records in 1usize..60, threshold in 0usize..60) {
        let file = ndjson(num_records);
        let loader = DuckdbConnection::open_path(file.path()).unwrap();

        let filtered: usize = loader
            .fetch(vec![format!("id < {threshold}")], None, None)
            .unwrap()
            .iter()
            .map(|b| b.num_rows())
            .sum();

        prop_assert_eq!(filtered, threshold.min(num_records));
    }
}
