//! Loading fixtures through the DuckDB query engine (#147/#148).
//!
//! Every format now reaches the app through one loader, so these exercise the
//! engine rather than a per-format reader: open a file, count its rows, and
//! read individual records back as JSON.

use std::path::Path;
use thoth::file::detect_file_type::{DetectedFileType, sniff_file_type};
use thoth::file::loaders::{DuckdbConnection, FileLoader, RecordSource};

fn open(path: &str) -> DuckdbConnection {
    DuckdbConnection::open_path(Path::new(path)).unwrap_or_else(|e| panic!("open {path}: {e}"))
}

#[test]
fn test_load_ndjson_simple_fixture() {
    let loader = open("tests/fixtures/ndjson/simple.ndjson");

    assert_eq!(loader.len().unwrap(), 10);

    let val = loader.record(0).unwrap();
    assert_eq!(val["id"], 1);
    assert_eq!(val["name"], "Alice");

    let val = loader.record(9).unwrap();
    assert_eq!(val["id"], 10);
    assert_eq!(val["name"], "Jack");
}

#[test]
fn test_load_ndjson_nested_fixture() {
    let loader = open("tests/fixtures/ndjson/nested.ndjson");

    assert_eq!(loader.len().unwrap(), 3);

    let val = loader.record(0).unwrap();
    assert_eq!(val["user"]["name"], "Alice");
    assert_eq!(val["user"]["address"]["city"], "NYC");
    assert!(val["tags"].is_array());
}

#[test]
fn test_load_ndjson_empty_fixture() {
    // An empty file has no schema to infer, so it either fails to open or
    // opens with no rows — never a phantom record.
    let path = Path::new("tests/fixtures/ndjson/empty.ndjson");
    if let Ok(loader) = DuckdbConnection::open_path(path) {
        assert_eq!(loader.len().unwrap_or(0), 0);
    }
}

#[test]
fn test_load_ndjson_single_line_fixture() {
    let loader = open("tests/fixtures/ndjson/single_line.ndjson");

    assert_eq!(loader.len().unwrap(), 1);
    assert_eq!(loader.record(0).unwrap()["id"], 1);
}

#[test]
fn test_load_json_array_simple_fixture() {
    let loader = open("tests/fixtures/json_array/simple.json");

    assert_eq!(loader.len().unwrap(), 3);

    let val = loader.record(0).unwrap();
    assert_eq!(val["id"], 1);
    assert_eq!(val["name"], "Alice");
}

#[test]
fn test_load_json_array_nested_fixture() {
    let loader = open("tests/fixtures/json_array/nested.json");

    assert_eq!(loader.len().unwrap(), 2);

    let val = loader.record(0).unwrap();
    assert_eq!(val["user"]["name"], "Alice");
    assert!(val["items"].is_array());
}

#[test]
fn test_load_json_array_empty_fixture() {
    let path = Path::new("tests/fixtures/json_array/empty.json");
    if let Ok(loader) = DuckdbConnection::open_path(path) {
        assert_eq!(loader.len().unwrap_or(0), 0);
    }
}

#[test]
fn test_load_json_array_single_element_fixture() {
    let loader = open("tests/fixtures/json_array/single_element.json");

    assert_eq!(loader.len().unwrap(), 1);
    assert_eq!(loader.record(0).unwrap()["id"], 1);
}

#[test]
fn test_load_json_object_simple_fixture() {
    // A single top-level object is one row whose columns are its keys.
    let loader = open("tests/fixtures/json_object/simple.json");

    assert_eq!(loader.len().unwrap(), 1);

    let val = loader.record(0).unwrap();
    assert_eq!(val["id"], 1);
    assert_eq!(val["name"], "Alice");
    assert_eq!(val["age"], 30);
}

#[test]
fn test_load_json_object_nested_fixture() {
    let loader = open("tests/fixtures/json_object/nested.json");

    let val = loader.record(0).unwrap();
    assert_eq!(val["user"]["name"], "Alice");
    assert_eq!(val["user"]["profile"]["address"]["city"], "NYC");
}

#[test]
fn test_load_json_object_empty_fixture() {
    // `{}` has no fields to infer a schema from, so DuckDB falls back to a
    // single catch-all `json` column rather than an empty row. Documenting the
    // real shape here: the row exists, and the object is nested one level
    // deeper than the file suggests.
    let loader = open("tests/fixtures/json_object/empty.json");

    assert_eq!(loader.len().unwrap(), 1);
    assert_eq!(loader.column_names().unwrap(), vec!["json"]);

    let val = loader.record(0).unwrap();
    assert!(val["json"].is_object());
    assert!(val["json"].as_object().unwrap().is_empty());
}

#[test]
fn test_detect_file_types() {
    let test_cases = vec![
        (
            "tests/fixtures/ndjson/simple.ndjson",
            DetectedFileType::Ndjson,
        ),
        (
            "tests/fixtures/json_array/simple.json",
            DetectedFileType::JsonArray,
        ),
        (
            "tests/fixtures/json_object/simple.json",
            DetectedFileType::JsonObject,
        ),
        (
            "tests/fixtures/json_array/empty.json",
            DetectedFileType::JsonArray,
        ),
        (
            "tests/fixtures/json_object/empty.json",
            DetectedFileType::JsonObject,
        ),
    ];

    for (path, expected) in test_cases {
        let detected = sniff_file_type(Path::new(path)).unwrap();
        assert_eq!(detected, expected, "Failed for {}", path);
    }
}

#[test]
fn test_edge_case_unicode() {
    let loader = open("tests/fixtures/edge_cases/unicode.json");

    let val = loader.record(0).unwrap();
    assert_eq!(val["name"], "José García");
    assert_eq!(val["emoji"], "🚀🎉");
    assert_eq!(val["chinese"], "你好世界");
}

#[test]
fn test_edge_case_escaped() {
    let loader = open("tests/fixtures/edge_cases/escaped.json");

    let val = loader.record(0).unwrap();
    assert_eq!(val["quote"], "He said \"Hello\"");
    assert_eq!(val["newline"], "Line1\nLine2");
}

#[test]
fn test_edge_case_numbers() {
    let loader = open("tests/fixtures/edge_cases/numbers.json");

    let val = loader.record(0).unwrap();
    assert_eq!(val["int"], 42);
    assert_eq!(val["negative"], -123);
    assert_eq!(val["zero"], 0);
}

#[test]
fn test_edge_case_mixed_types() {
    let loader = open("tests/fixtures/edge_cases/mixed_types.json");

    let val = loader.record(0).unwrap();
    assert!(val["string"].is_string());
    assert!(val["number"].is_number());
    assert!(val["boolean_true"].is_boolean());
    assert!(val["null_value"].is_null());
    assert!(val["array"].is_array());
    assert!(val["object"].is_object());
    assert_eq!(val["empty_string"], "");
}

#[test]
fn test_random_access_ndjson() {
    let loader = open("tests/fixtures/ndjson/simple.ndjson");

    // Access in non-sequential order
    assert_eq!(loader.record(5).unwrap()["id"], 6);
    assert_eq!(loader.record(2).unwrap()["id"], 3);
    assert_eq!(loader.record(8).unwrap()["id"], 9);

    // Access the same index again
    assert_eq!(loader.record(5).unwrap()["id"], 6);
}

#[test]
fn test_raw_bytes_access() {
    let loader = open("tests/fixtures/ndjson/simple.ndjson");

    let raw = loader.raw_bytes(0).unwrap();
    let s = String::from_utf8(raw).unwrap();
    assert!(s.contains("\"id\":1"), "got: {s}");
    assert!(s.contains("\"name\":\"Alice\""), "got: {s}");
}

#[test]
fn test_fetch_pushes_predicates_into_the_scan() {
    // The point of the engine: filtering happens in DuckDB, not in Rust.
    let loader = open("tests/fixtures/ndjson/simple.ndjson");

    let matching: usize = loader
        .fetch(vec!["id > 7".to_string()], None, None)
        .unwrap()
        .iter()
        .map(|b| b.num_rows())
        .sum();
    assert_eq!(matching, 3);
}

#[test]
fn test_sql_runs_against_the_file() {
    let loader = open("tests/fixtures/ndjson/simple.ndjson");
    let alias = loader.primary_alias().unwrap();

    let batches = loader
        .query(&format!(
            "SELECT count(*) AS n FROM \"{alias}\" WHERE id <= 4"
        ))
        .unwrap();
    let rows = thoth::file::loaders::batches_to_values(&batches).unwrap();
    assert_eq!(rows[0]["n"], 4);
}
