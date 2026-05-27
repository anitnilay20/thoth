//! Comprehensive tests for the MCP server module.
//!
//! Tests cover: ServerState, tool parameter types, and end-to-end tool logic.

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    use crate::mcp::state::ServerState;

    // ─── Helper: create a temp NDJSON file ────────────────────────────────

    fn create_ndjson_file(lines: &[&str]) -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
        f.flush().unwrap();
        f
    }

    fn create_json_array_file(records: &[&str]) -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        write!(f, "[").unwrap();
        for (i, rec) in records.iter().enumerate() {
            if i > 0 {
                write!(f, ",").unwrap();
            }
            write!(f, "{}", rec).unwrap();
        }
        write!(f, "]").unwrap();
        f.flush().unwrap();
        f
    }

    fn create_json_object_file(json: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        write!(f, "{}", json).unwrap();
        f.flush().unwrap();
        f
    }

    // ─── ServerState tests ───────────────────────────────────────────────

    #[test]
    fn test_state_open_and_close_ndjson() {
        let file = create_ndjson_file(&[
            r#"{"name":"alice","age":30}"#,
            r#"{"name":"bob","age":25}"#,
            r#"{"name":"carol","age":35}"#,
        ]);

        let state = ServerState::new();
        let (handle, info) = state.open_file(file.path()).expect("open_file failed");

        assert!(!handle.is_empty());
        assert_eq!(info.file_type, "ndjson");
        assert_eq!(info.record_count, 3);

        // file_info should return the same data
        let info2 = state.file_info(&handle).expect("file_info failed");
        assert_eq!(info2.record_count, 3);
        assert_eq!(info2.file_type, "ndjson");

        // close
        assert!(state.close_file(&handle));
        assert!(!state.close_file(&handle)); // double close returns false
    }

    #[test]
    fn test_state_open_json_array() {
        let file = create_json_array_file(&[
            r#"{"id":1}"#,
            r#"{"id":2}"#,
        ]);

        let state = ServerState::new();
        let (_handle, info) = state.open_file(file.path()).expect("open_file failed");

        assert_eq!(info.file_type, "json_array");
        assert_eq!(info.record_count, 2);
    }

    #[test]
    fn test_state_open_json_object() {
        let file = create_json_object_file(r#"{"key":"value","nested":{"a":1}}"#);

        let state = ServerState::new();
        let (_handle, info) = state.open_file(file.path()).expect("open_file failed");

        assert_eq!(info.file_type, "json_object");
        assert_eq!(info.record_count, 1);
    }

    #[test]
    fn test_state_open_nonexistent_file() {
        let state = ServerState::new();
        let result = state.open_file(&PathBuf::from("/nonexistent/path/to/file.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_state_with_file_get_record() {
        let file = create_ndjson_file(&[
            r#"{"name":"alice"}"#,
            r#"{"name":"bob"}"#,
        ]);

        let state = ServerState::new();
        let (handle, _) = state.open_file(file.path()).unwrap();

        let value = state.with_file(&handle, |f| {
            f.file_type.get(0).unwrap()
        });

        assert!(value.is_some());
        let val = value.unwrap();
        assert_eq!(val["name"], "alice");
    }

    #[test]
    fn test_state_with_file_nonexistent_handle() {
        let state = ServerState::new();
        let result = state.with_file("nonexistent", |_| 42);
        assert!(result.is_none());
    }

    #[test]
    fn test_state_multiple_files() {
        let file1 = create_ndjson_file(&[r#"{"a":1}"#, r#"{"a":2}"#]);
        let file2 = create_json_array_file(&[r#"{"b":1}"#, r#"{"b":2}"#, r#"{"b":3}"#]);

        let state = ServerState::new();
        let (h1, info1) = state.open_file(file1.path()).unwrap();
        let (h2, info2) = state.open_file(file2.path()).unwrap();

        assert_ne!(h1, h2);
        assert_eq!(info1.record_count, 2);
        assert_eq!(info2.record_count, 3);

        // Both should be accessible
        let handles = state.list_handles();
        assert_eq!(handles.len(), 2);

        // Close one, other should still work
        state.close_file(&h1);
        assert!(state.file_info(&h1).is_none());
        assert!(state.file_info(&h2).is_some());
    }

    #[test]
    fn test_state_unique_handles() {
        let file = create_ndjson_file(&[r#"{"a":1}"#]);

        let state = ServerState::new();
        let (h1, _) = state.open_file(file.path()).unwrap();
        let (h2, _) = state.open_file(file.path()).unwrap();

        // Same file opened twice should get different handles
        assert_ne!(h1, h2);
    }

    // ─── Search integration tests ────────────────────────────────────────

    #[test]
    fn test_search_text_mode() {
        use crate::search::{QueryMode, Search};
        use crate::file::loaders::FileKind;

        let file = create_ndjson_file(&[
            r#"{"name":"alice","city":"wonderland"}"#,
            r#"{"name":"bob","city":"springfield"}"#,
            r#"{"name":"alice","city":"new york"}"#,
        ]);

        let mut search = Search {
            query: "alice".to_string(),
            match_case: false,
            query_mode: QueryMode::Text,
            ..Search::default()
        };

        let path_opt = Some(file.path().to_path_buf());
        search.start_scanning_internal(&path_opt, &FileKind::Ndjson);

        assert!(search.error.is_none(), "Search had error: {:?}", search.error);
        assert_eq!(search.results.len(), 2);
        assert!(!search.scanning);
    }

    #[test]
    fn test_search_jsonpath_mode() {
        use crate::search::{QueryMode, Search};
        use crate::file::loaders::FileKind;

        let file = create_ndjson_file(&[
            r#"{"user":{"name":"alice","age":30}}"#,
            r#"{"user":{"name":"bob","age":25}}"#,
            r#"{"user":{"name":"carol","age":35}}"#,
        ]);

        let mut search = Search {
            query: "$.user.name".to_string(),
            match_case: false,
            query_mode: QueryMode::JsonPath,
            ..Search::default()
        };

        let path_opt = Some(file.path().to_path_buf());
        search.start_scanning_internal(&path_opt, &FileKind::Ndjson);

        assert!(search.error.is_none(), "Search had error: {:?}", search.error);
        // All 3 records have $.user.name
        assert_eq!(search.results.len(), 3);
    }

    #[test]
    fn test_search_jsonpath_with_filter() {
        use crate::search::{QueryMode, Search};
        use crate::file::loaders::FileKind;

        let file = create_ndjson_file(&[
            r#"{"user":{"name":"alice","age":30}}"#,
            r#"{"user":{"name":"bob","age":25}}"#,
            r#"{"user":{"name":"carol","age":35}}"#,
        ]);

        let mut search = Search {
            query: "$.user.name = \"alice\"".to_string(),
            match_case: false,
            query_mode: QueryMode::JsonPath,
            ..Search::default()
        };

        let path_opt = Some(file.path().to_path_buf());
        search.start_scanning_internal(&path_opt, &FileKind::Ndjson);

        assert!(search.error.is_none(), "Search had error: {:?}", search.error);
        assert_eq!(search.results.len(), 1);
        assert_eq!(search.results.hits()[0].record_index, 0);
    }

    #[test]
    fn test_search_case_sensitive() {
        use crate::search::{QueryMode, Search};
        use crate::file::loaders::FileKind;

        let file = create_ndjson_file(&[
            r#"{"name":"Alice"}"#,
            r#"{"name":"alice"}"#,
        ]);

        // Case-sensitive: should find only lowercase
        let mut search = Search {
            query: "alice".to_string(),
            match_case: true,
            query_mode: QueryMode::Text,
            ..Search::default()
        };

        let path_opt = Some(file.path().to_path_buf());
        search.start_scanning_internal(&path_opt, &FileKind::Ndjson);

        assert!(search.error.is_none());
        assert_eq!(search.results.len(), 1);
        assert_eq!(search.results.hits()[0].record_index, 1);
    }

    #[test]
    fn test_search_empty_query() {
        use crate::search::{QueryMode, Search};
        use crate::file::loaders::FileKind;

        let file = create_ndjson_file(&[r#"{"name":"alice"}"#]);

        let mut search = Search {
            query: String::new(),
            match_case: false,
            query_mode: QueryMode::Text,
            ..Search::default()
        };

        let path_opt = Some(file.path().to_path_buf());
        search.start_scanning_internal(&path_opt, &FileKind::Ndjson);

        assert!(search.error.is_none());
        assert_eq!(search.results.len(), 0);
    }

    #[test]
    fn test_search_no_matches() {
        use crate::search::{QueryMode, Search};
        use crate::file::loaders::FileKind;

        let file = create_ndjson_file(&[
            r#"{"name":"alice"}"#,
            r#"{"name":"bob"}"#,
        ]);

        let mut search = Search {
            query: "zzz_nonexistent".to_string(),
            match_case: false,
            query_mode: QueryMode::Text,
            ..Search::default()
        };

        let path_opt = Some(file.path().to_path_buf());
        search.start_scanning_internal(&path_opt, &FileKind::Ndjson);

        assert!(search.error.is_none());
        assert_eq!(search.results.len(), 0);
    }

    // ─── End-to-end tool logic tests ─────────────────────────────────────

    #[test]
    fn test_tool_open_close_roundtrip() {
        let file = create_ndjson_file(&[
            r#"{"a":1}"#,
            r#"{"a":2}"#,
        ]);

        let state = ServerState::new();
        let (handle, info) = state.open_file(file.path()).unwrap();

        assert_eq!(info.record_count, 2);

        // Get record
        let record = state.with_file(&handle, |f| {
            f.file_type.get(1).unwrap()
        }).unwrap();
        assert_eq!(record["a"], 2);

        // Close
        assert!(state.close_file(&handle));

        // Verify closed
        assert!(state.file_info(&handle).is_none());
    }

    #[test]
    fn test_tool_get_record_out_of_bounds() {
        let file = create_ndjson_file(&[r#"{"x":1}"#]);

        let state = ServerState::new();
        let (handle, _) = state.open_file(file.path()).unwrap();

        let result = state.with_file(&handle, |f| {
            f.file_type.get(999)
        });

        assert!(result.is_some());
        assert!(result.unwrap().is_err());
    }

    // ─── Fixture-based tests ─────────────────────────────────────────────

    #[test]
    fn test_open_fixture_simple_ndjson() {
        let path = PathBuf::from("tests/fixtures/ndjson/simple.ndjson");
        if !path.exists() {
            return; // Skip if fixtures not available
        }

        let state = ServerState::new();
        let (handle, info) = state.open_file(&path).unwrap();

        assert_eq!(info.file_type, "ndjson");
        assert!(info.record_count > 0);

        // Should be able to get first record
        let val = state.with_file(&handle, |f| f.file_type.get(0).unwrap()).unwrap();
        assert!(val.is_object());
    }

    #[test]
    fn test_open_fixture_simple_json_array() {
        let path = PathBuf::from("tests/fixtures/json_array/simple.json");
        if !path.exists() {
            return;
        }

        let state = ServerState::new();
        let (_handle, info) = state.open_file(&path).unwrap();

        assert_eq!(info.file_type, "json_array");
        assert!(info.record_count > 0);
    }

    #[test]
    fn test_open_fixture_simple_json_object() {
        let path = PathBuf::from("tests/fixtures/json_object/simple.json");
        if !path.exists() {
            return;
        }

        let state = ServerState::new();
        let (_handle, info) = state.open_file(&path).unwrap();

        assert_eq!(info.file_type, "json_object");
        assert_eq!(info.record_count, 1);
    }

    #[test]
    fn test_open_fixture_nested_ndjson() {
        let path = PathBuf::from("tests/fixtures/ndjson/nested.ndjson");
        if !path.exists() {
            return;
        }

        let state = ServerState::new();
        let (handle, info) = state.open_file(&path).unwrap();

        assert_eq!(info.file_type, "ndjson");

        // Verify we can get records with nested structures
        let val = state.with_file(&handle, |f| f.file_type.get(0).unwrap()).unwrap();
        assert!(val.is_object());
    }

    #[test]
    fn test_search_across_fixture_ndjson() {
        use crate::search::{QueryMode, Search};

        let path = PathBuf::from("tests/fixtures/ndjson/simple.ndjson");
        if !path.exists() {
            return;
        }

        let state = ServerState::new();
        let (handle, info) = state.open_file(&path).unwrap();

        // Do a text search using the Search engine directly
        let mut search = Search {
            query: "name".to_string(), // generic field likely present
            match_case: false,
            query_mode: QueryMode::Text,
            ..Search::default()
        };

        let path_opt = Some(path);
        let file_kind = state.with_file(&handle, |f| f.file_kind).unwrap_or_default();
        search.start_scanning_internal(&path_opt, &file_kind);

        assert!(search.error.is_none(), "Search error: {:?}", search.error);
        // Should find at least one match (the word "name" appears as a key)
        assert!(search.results.len() > 0 || info.record_count == 0);
    }

    // ─── Edge case tests ─────────────────────────────────────────────────

    #[test]
    fn test_open_fixture_edge_unicode() {
        let path = PathBuf::from("tests/fixtures/edge_cases/unicode.json");
        if !path.exists() {
            return;
        }

        let state = ServerState::new();
        let result = state.open_file(&path);
        assert!(result.is_ok(), "Should handle unicode JSON: {:?}", result.err());
    }

    #[test]
    fn test_open_fixture_edge_escaped() {
        let path = PathBuf::from("tests/fixtures/edge_cases/escaped.json");
        if !path.exists() {
            return;
        }

        let state = ServerState::new();
        let result = state.open_file(&path);
        assert!(result.is_ok(), "Should handle escaped JSON: {:?}", result.err());
    }

    #[test]
    fn test_open_fixture_edge_numbers() {
        let path = PathBuf::from("tests/fixtures/edge_cases/numbers.json");
        if !path.exists() {
            return;
        }

        let state = ServerState::new();
        let result = state.open_file(&path);
        assert!(result.is_ok(), "Should handle numbers JSON: {:?}", result.err());
    }

    // ─── Phase 2: Data tool tests ───────────────────────────────────────

    #[test]
    fn test_get_value_at_path_simple() {
        let file = create_ndjson_file(&[
            r#"{"user":{"name":"alice","address":{"city":"wonderland","zip":"12345"}}}"#,
        ]);

        let state = ServerState::new();
        let (handle, _) = state.open_file(file.path()).unwrap();

        // Get nested value
        let result = state.with_file(&handle, |f| {
            let record = f.file_type.get(0).unwrap();
            crate::helpers::walk_rel(record, "user.address.city")
        }).unwrap().unwrap();

        assert_eq!(result, serde_json::json!("wonderland"));
    }

    #[test]
    fn test_get_value_at_path_array_index() {
        let file = create_ndjson_file(&[
            r#"{"items":[{"name":"a"},{"name":"b"},{"name":"c"}]}"#,
        ]);

        let state = ServerState::new();
        let (handle, _) = state.open_file(file.path()).unwrap();

        let result = state.with_file(&handle, |f| {
            let record = f.file_type.get(0).unwrap();
            crate::helpers::walk_rel(record, "items[1].name")
        }).unwrap().unwrap();

        assert_eq!(result, serde_json::json!("b"));
    }

    #[test]
    fn test_get_value_at_path_invalid_path() {
        let file = create_ndjson_file(&[
            r#"{"name":"alice"}"#,
        ]);

        let state = ServerState::new();
        let (handle, _) = state.open_file(file.path()).unwrap();

        let result = state.with_file(&handle, |f| {
            let record = f.file_type.get(0).unwrap();
            crate::helpers::walk_rel(record, "nonexistent.path")
        }).unwrap();

        assert!(result.is_err());
    }

    #[test]
    fn test_get_value_at_path_empty_path() {
        let file = create_ndjson_file(&[
            r#"{"name":"alice"}"#,
        ]);

        let state = ServerState::new();
        let (handle, _) = state.open_file(file.path()).unwrap();

        // Empty path should return entire record
        let result = state.with_file(&handle, |f| {
            let record = f.file_type.get(0).unwrap();
            crate::helpers::walk_rel(record.clone(), "")
        }).unwrap().unwrap();

        assert_eq!(result["name"], "alice");
    }

    #[test]
    fn test_extract_keys_top_level() {
        use std::collections::BTreeSet;

        let file = create_ndjson_file(&[
            r#"{"name":"alice","age":30,"city":"wonderland"}"#,
            r#"{"name":"bob","age":25,"email":"bob@test.com"}"#,
            r#"{"name":"carol","score":95}"#,
        ]);

        let state = ServerState::new();
        let (handle, _) = state.open_file(file.path()).unwrap();

        let keys = state.with_file(&handle, |f| {
            let mut all_keys = BTreeSet::new();
            for i in 0..f.record_count() {
                if let Ok(record) = f.file_type.get(i) {
                    if let Some(obj) = record.as_object() {
                        for key in obj.keys() {
                            all_keys.insert(key.clone());
                        }
                    }
                }
            }
            all_keys.into_iter().collect::<Vec<_>>()
        }).unwrap();

        assert!(keys.contains(&"name".to_string()));
        assert!(keys.contains(&"age".to_string()));
        assert!(keys.contains(&"city".to_string()));
        assert!(keys.contains(&"email".to_string()));
        assert!(keys.contains(&"score".to_string()));
        assert_eq!(keys.len(), 5);
    }

    #[test]
    fn test_extract_keys_nested_path() {
        let file = create_ndjson_file(&[
            r#"{"user":{"name":"alice","age":30}}"#,
            r#"{"user":{"name":"bob","email":"bob@test.com"}}"#,
        ]);

        let state = ServerState::new();
        let (handle, _) = state.open_file(file.path()).unwrap();

        let keys = state.with_file(&handle, |f| {
            use std::collections::BTreeSet;
            let mut all_keys = BTreeSet::new();
            for i in 0..f.record_count() {
                if let Ok(record) = f.file_type.get(i) {
                    if let Ok(nested) = crate::helpers::walk_rel(record, "user") {
                        if let Some(obj) = nested.as_object() {
                            for key in obj.keys() {
                                all_keys.insert(key.clone());
                            }
                        }
                    }
                }
            }
            all_keys.into_iter().collect::<Vec<_>>()
        }).unwrap();

        assert!(keys.contains(&"name".to_string()));
        assert!(keys.contains(&"age".to_string()));
        assert!(keys.contains(&"email".to_string()));
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn test_sample_records_first() {
        let file = create_ndjson_file(&[
            r#"{"id":1}"#, r#"{"id":2}"#, r#"{"id":3}"#,
            r#"{"id":4}"#, r#"{"id":5}"#,
        ]);

        let state = ServerState::new();
        let (handle, _) = state.open_file(file.path()).unwrap();

        let records = state.with_file(&handle, |f| {
            let n = 3.min(f.record_count());
            (0..n).map(|i| f.file_type.get(i).unwrap()).collect::<Vec<_>>()
        }).unwrap();

        assert_eq!(records.len(), 3);
        assert_eq!(records[0]["id"], 1);
        assert_eq!(records[2]["id"], 3);
    }

    #[test]
    fn test_sample_records_last() {
        let file = create_ndjson_file(&[
            r#"{"id":1}"#, r#"{"id":2}"#, r#"{"id":3}"#,
            r#"{"id":4}"#, r#"{"id":5}"#,
        ]);

        let state = ServerState::new();
        let (handle, _) = state.open_file(file.path()).unwrap();

        let total = state.with_file(&handle, |f| f.record_count()).unwrap();
        let n = 2usize;
        let start = total.saturating_sub(n);
        let records = state.with_file(&handle, |f| {
            (start..total).map(|i| f.file_type.get(i).unwrap()).collect::<Vec<_>>()
        }).unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["id"], 4);
        assert_eq!(records[1]["id"], 5);
    }

    #[test]
    fn test_sample_records_even() {
        let file = create_ndjson_file(&[
            r#"{"id":0}"#, r#"{"id":1}"#, r#"{"id":2}"#, r#"{"id":3}"#,
            r#"{"id":4}"#, r#"{"id":5}"#, r#"{"id":6}"#, r#"{"id":7}"#,
            r#"{"id":8}"#, r#"{"id":9}"#,
        ]);

        let state = ServerState::new();
        let (handle, _) = state.open_file(file.path()).unwrap();

        let total = state.with_file(&handle, |f| f.record_count()).unwrap();
        assert_eq!(total, 10);

        let n = 3usize;
        let indices: Vec<usize> = (0..n)
            .map(|i| i * (total - 1) / (n - 1).max(1))
            .collect();

        // Should give evenly spaced: 0, 4 (or 5), 9
        assert_eq!(indices[0], 0);
        assert_eq!(*indices.last().unwrap(), 9);
        assert_eq!(indices.len(), 3);
    }

    #[test]
    fn test_infer_schema_uniform_objects() {
        use crate::mcp::tools::infer_schema;

        let samples = vec![
            serde_json::json!({"name": "alice", "age": 30}),
            serde_json::json!({"name": "bob", "age": 25}),
        ];

        let schema = infer_schema(&samples);

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["name"].is_object());
        assert_eq!(schema["properties"]["name"]["type"], "string");
        assert_eq!(schema["properties"]["age"]["type"], "number");

        // Both fields present in all records => required
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 2);
    }

    #[test]
    fn test_infer_schema_optional_fields() {
        use crate::mcp::tools::infer_schema;

        let samples = vec![
            serde_json::json!({"name": "alice", "age": 30}),
            serde_json::json!({"name": "bob"}),
        ];

        let schema = infer_schema(&samples);

        assert_eq!(schema["type"], "object");
        // "name" is required, "age" is not
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::json!("name")));
        assert!(!required.contains(&serde_json::json!("age")));
    }

    #[test]
    fn test_infer_schema_mixed_types() {
        use crate::mcp::tools::infer_schema;

        let samples = vec![
            serde_json::json!({"value": "text"}),
            serde_json::json!({"value": 42}),
        ];

        let schema = infer_schema(&samples);

        assert_eq!(schema["type"], "object");
        // "value" should have multiple types
        let val_type = &schema["properties"]["value"]["type"];
        assert!(val_type.is_array());
    }

    #[test]
    fn test_infer_schema_array() {
        use crate::mcp::tools::infer_schema;

        let samples = vec![
            serde_json::json!([1, 2, 3]),
            serde_json::json!([4, 5]),
        ];

        let schema = infer_schema(&samples);
        assert_eq!(schema["type"], "array");
        assert_eq!(schema["items"]["type"], "number");
    }

    #[test]
    fn test_infer_schema_empty() {
        use crate::mcp::tools::infer_schema;

        let samples: Vec<serde_json::Value> = vec![];
        let schema = infer_schema(&samples);
        assert_eq!(schema["type"], "unknown");
    }

    #[test]
    fn test_infer_schema_from_fixture_ndjson() {
        use crate::mcp::tools::infer_schema;

        let path = PathBuf::from("tests/fixtures/ndjson/simple.ndjson");
        if !path.exists() { return; }

        let state = ServerState::new();
        let (handle, _) = state.open_file(&path).unwrap();

        let schema = state.with_file(&handle, |f| {
            let count = f.record_count().min(10);
            let mut samples = Vec::new();
            for i in 0..count {
                if let Ok(v) = f.file_type.get(i) { samples.push(v); }
            }
            infer_schema(&samples)
        }).unwrap();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
    }

    // ─── Concurrency test ────────────────────────────────────────────────

    #[test]
    fn test_concurrent_state_access() {
        use std::thread;

        let file = create_ndjson_file(&[
            r#"{"name":"alice"}"#,
            r#"{"name":"bob"}"#,
        ]);

        let state = ServerState::new();
        let (handle, _) = state.open_file(file.path()).unwrap();

        // Spawn threads that all read from the same state
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let state = state.clone();
                let handle = handle.clone();
                thread::spawn(move || {
                    for _ in 0..10 {
                        let info = state.file_info(&handle);
                        assert!(info.is_some());
                        assert_eq!(info.unwrap().record_count, 2);
                    }
                })
            })
            .collect();

        for t in threads {
            t.join().unwrap();
        }
    }
}
