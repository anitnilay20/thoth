use proptest::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;
use thoth::file::loaders::{FileLoader, JsonArrayFile, NdjsonFile, SingleValueFile};

// Property: For any number of NDJSON records, len() should equal the number of records
proptest! {
    #[test]
    fn test_ndjson_len_matches_records(num_records in 1usize..100) {
        let mut file = NamedTempFile::new().unwrap();
        for i in 0..num_records {
            writeln!(file, r#"{{"id":{}}}"#, i).unwrap();
        }
        file.flush().unwrap();

        let loader = NdjsonFile::open(file.path()).unwrap();
        prop_assert_eq!(loader.len(), num_records);
    }

    // Property: All valid indices should succeed, out of bounds should fail
    #[test]
    fn test_ndjson_bounds_invariant(num_records in 1usize..50) {
        let mut file = NamedTempFile::new().unwrap();
        for i in 0..num_records {
            writeln!(file, r#"{{"id":{}}}"#, i).unwrap();
        }
        file.flush().unwrap();

        let mut loader = NdjsonFile::open(file.path()).unwrap();

        // All valid indices should succeed
        for i in 0..num_records {
            prop_assert!(loader.get(i).is_ok());
        }

        // Out of bounds should fail
        prop_assert!(loader.get(num_records).is_err());
        prop_assert!(loader.get(num_records + 100).is_err());
    }

    // Property: raw_bytes should return valid JSON
    #[test]
    fn test_ndjson_raw_bytes_valid_json(num_records in 1usize..20) {
        let mut file = NamedTempFile::new().unwrap();
        for i in 0..num_records {
            writeln!(file, "{{\"id\":{},\"value\":\"record_{}\"}}", i, i).unwrap();
        }
        file.flush().unwrap();

        let loader = NdjsonFile::open(file.path()).unwrap();

        // Every raw_bytes should be parseable JSON
        for i in 0..num_records {
            let raw = loader.raw_bytes(i).unwrap();
            let parsed: Result<serde_json::Value, _> = serde_json::from_slice(&raw);
            prop_assert!(parsed.is_ok(), "Failed to parse raw bytes at index {}: {:?}", i, String::from_utf8_lossy(&raw));
        }
    }

    // Property: get() results should match raw_bytes when parsed
    #[test]
    fn test_ndjson_get_matches_raw_bytes(num_records in 1usize..20, idx in 0usize..19) {
        let num_records = num_records.max(idx + 1); // Ensure idx is valid

        let mut file = NamedTempFile::new().unwrap();
        for i in 0..num_records {
            writeln!(file, r#"{{"id":{}}}"#, i).unwrap();
        }
        file.flush().unwrap();

        let mut loader = NdjsonFile::open(file.path()).unwrap();

        let parsed = loader.get(idx).unwrap();
        let raw = loader.raw_bytes(idx).unwrap();
        let from_raw: serde_json::Value = serde_json::from_slice(&raw).unwrap();

        prop_assert_eq!(parsed, from_raw);
    }

    // Property: JSON array with N elements should have len() == N
    #[test]
    fn test_json_array_len_invariant(num_elements in 1usize..50) {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "[").unwrap();
        for i in 0..num_elements {
            write!(file, r#"{{"id":{}}}"#, i).unwrap();
            if i < num_elements - 1 {
                write!(file, ",").unwrap();
            }
        }
        write!(file, "]").unwrap();
        file.flush().unwrap();

        let loader = JsonArrayFile::open(file.path()).unwrap();
        prop_assert_eq!(loader.len(), num_elements);
    }

    // Property: All indices < len() should be accessible
    #[test]
    fn test_json_array_all_indices_accessible(num_elements in 1usize..30) {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "[").unwrap();
        for i in 0..num_elements {
            write!(file, r#"{{"id":{}}}"#, i).unwrap();
            if i < num_elements - 1 {
                write!(file, ",").unwrap();
            }
        }
        write!(file, "]").unwrap();
        file.flush().unwrap();

        let mut loader = JsonArrayFile::open(file.path()).unwrap();

        for i in 0..num_elements {
            let val = loader.get(i).unwrap();
            prop_assert_eq!(val["id"].as_u64().unwrap() as usize, i);
        }
    }

    // Property: SingleValueFile always has len() == 1
    #[test]
    fn test_single_value_len_always_one(value in 0i32..1000) {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"{{"value":{}}}"#, value).unwrap();
        file.flush().unwrap();

        let loader = SingleValueFile::open(file.path()).unwrap();
        prop_assert_eq!(loader.len(), 1);
        prop_assert!(!loader.is_empty());
    }

    // Property: SingleValueFile get(0) always succeeds, get(n>0) always fails
    #[test]
    fn test_single_value_index_invariant(value in 0i32..100, invalid_idx in 1usize..100) {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"{{"value":{}}}"#, value).unwrap();
        file.flush().unwrap();

        let mut loader = SingleValueFile::open(file.path()).unwrap();

        // Index 0 should always succeed
        prop_assert!(loader.get(0).is_ok());

        // Any index > 0 should fail
        prop_assert!(loader.get(invalid_idx).is_err());
    }

    // Property: Repeated get() calls should return same value (caching correctness)
    #[test]
    fn test_single_value_caching_consistent(value in 0i32..100) {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"{{"value":{}}}"#, value).unwrap();
        file.flush().unwrap();

        let mut loader = SingleValueFile::open(file.path()).unwrap();

        let val1 = loader.get(0).unwrap();
        let val2 = loader.get(0).unwrap();
        let val3 = loader.get(0).unwrap();

        prop_assert_eq!(&val1, &val2);
        prop_assert_eq!(&val2, &val3);
        prop_assert_eq!(val1["value"].as_i64().unwrap(), value as i64);
    }

    // Property: is_empty() iff len() == 0
    #[test]
    fn test_is_empty_iff_len_zero(num_records in 0usize..50) {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "[").unwrap();
        for i in 0..num_records {
            write!(file, r#"{{"id":{}}}"#, i).unwrap();
            if i < num_records - 1 {
                write!(file, ",").unwrap();
            }
        }
        write!(file, "]").unwrap();
        file.flush().unwrap();

        let loader = JsonArrayFile::open(file.path()).unwrap();

        if num_records == 0 {
            prop_assert!(loader.is_empty());
        } else {
            prop_assert!(!loader.is_empty());
        }
        prop_assert_eq!(loader.is_empty(), loader.len() == 0);
    }
}
