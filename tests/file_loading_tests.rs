use std::path::Path;
use thoth::file::detect_file_type::{DetectedFileType, sniff_file_type};
use thoth::file::loaders::{FileLoader, LazyJsonFile};

#[test]
fn test_load_ndjson_simple_fixture() {
    let path = Path::new("tests/fixtures/ndjson/simple.ndjson");
    let mut loader = LazyJsonFile::open(path).unwrap();

    assert_eq!(loader.len(), 10);

    let val = loader.get(0).unwrap();
    assert_eq!(val["id"], 1);
    assert_eq!(val["name"], "Alice");

    let val = loader.get(9).unwrap();
    assert_eq!(val["id"], 10);
    assert_eq!(val["name"], "Jack");
}

#[test]
fn test_load_ndjson_nested_fixture() {
    let path = Path::new("tests/fixtures/ndjson/nested.ndjson");
    let mut loader = LazyJsonFile::open(path).unwrap();

    assert_eq!(loader.len(), 3);

    let val = loader.get(0).unwrap();
    assert_eq!(val["user"]["name"], "Alice");
    assert_eq!(val["user"]["address"]["city"], "NYC");
    assert!(val["tags"].is_array());
}

#[test]
fn test_load_ndjson_empty_fixture() {
    let path = Path::new("tests/fixtures/ndjson/empty.ndjson");
    // Empty files cannot be automatically detected, so this should fail
    let result = LazyJsonFile::open(path);
    assert!(result.is_err());
}

#[test]
fn test_load_ndjson_single_line_fixture() {
    let path = Path::new("tests/fixtures/ndjson/single_line.ndjson");
    let mut loader = LazyJsonFile::open(path).unwrap();

    assert_eq!(loader.len(), 1);

    let val = loader.get(0).unwrap();
    assert_eq!(val["id"], 1);
}

#[test]
fn test_load_json_array_simple_fixture() {
    let path = Path::new("tests/fixtures/json_array/simple.json");
    let mut loader = LazyJsonFile::open(path).unwrap();

    assert_eq!(loader.len(), 3);

    let val = loader.get(0).unwrap();
    assert_eq!(val["id"], 1);
    assert_eq!(val["name"], "Alice");
}

#[test]
fn test_load_json_array_nested_fixture() {
    let path = Path::new("tests/fixtures/json_array/nested.json");
    let mut loader = LazyJsonFile::open(path).unwrap();

    assert_eq!(loader.len(), 2);

    let val = loader.get(0).unwrap();
    assert_eq!(val["user"]["name"], "Alice");
    assert!(val["items"].is_array());
}

#[test]
fn test_load_json_array_empty_fixture() {
    let path = Path::new("tests/fixtures/json_array/empty.json");
    let loader = LazyJsonFile::open(path).unwrap();

    assert_eq!(loader.len(), 0);
}

#[test]
fn test_load_json_array_single_element_fixture() {
    let path = Path::new("tests/fixtures/json_array/single_element.json");
    let mut loader = LazyJsonFile::open(path).unwrap();

    assert_eq!(loader.len(), 1);

    let val = loader.get(0).unwrap();
    assert_eq!(val["id"], 1);
}

#[test]
fn test_load_json_object_simple_fixture() {
    let path = Path::new("tests/fixtures/json_object/simple.json");
    let mut loader = LazyJsonFile::open(path).unwrap();

    assert_eq!(loader.len(), 1);

    let val = loader.get(0).unwrap();
    assert_eq!(val["id"], 1);
    assert_eq!(val["name"], "Alice");
    assert_eq!(val["age"], 30);
}

#[test]
fn test_load_json_object_nested_fixture() {
    let path = Path::new("tests/fixtures/json_object/nested.json");
    let mut loader = LazyJsonFile::open(path).unwrap();

    let val = loader.get(0).unwrap();
    assert_eq!(val["user"]["name"], "Alice");
    assert_eq!(val["user"]["profile"]["address"]["city"], "NYC");
}

#[test]
fn test_load_json_object_empty_fixture() {
    let path = Path::new("tests/fixtures/json_object/empty.json");
    let mut loader = LazyJsonFile::open(path).unwrap();

    let val = loader.get(0).unwrap();
    assert!(val.is_object());
    assert_eq!(val.as_object().unwrap().len(), 0);
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
    let path = Path::new("tests/fixtures/edge_cases/unicode.json");
    let mut loader = LazyJsonFile::open(path).unwrap();

    let val = loader.get(0).unwrap();
    assert_eq!(val["name"], "José García");
    assert_eq!(val["emoji"], "🚀🎉");
    assert_eq!(val["chinese"], "你好世界");
}

#[test]
fn test_edge_case_escaped() {
    let path = Path::new("tests/fixtures/edge_cases/escaped.json");
    let mut loader = LazyJsonFile::open(path).unwrap();

    let val = loader.get(0).unwrap();
    assert_eq!(val["quote"], "He said \"Hello\"");
    assert_eq!(val["newline"], "Line1\nLine2");
}

#[test]
fn test_edge_case_numbers() {
    let path = Path::new("tests/fixtures/edge_cases/numbers.json");
    let mut loader = LazyJsonFile::open(path).unwrap();

    let val = loader.get(0).unwrap();
    assert_eq!(val["int"], 42);
    assert_eq!(val["negative"], -123);
    assert_eq!(val["zero"], 0);
}

#[test]
fn test_edge_case_mixed_types() {
    let path = Path::new("tests/fixtures/edge_cases/mixed_types.json");
    let mut loader = LazyJsonFile::open(path).unwrap();

    let val = loader.get(0).unwrap();
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
    let path = Path::new("tests/fixtures/ndjson/simple.ndjson");
    let mut loader = LazyJsonFile::open(path).unwrap();

    // Access in non-sequential order
    let val5 = loader.get(5).unwrap();
    assert_eq!(val5["id"], 6);

    let val2 = loader.get(2).unwrap();
    assert_eq!(val2["id"], 3);

    let val8 = loader.get(8).unwrap();
    assert_eq!(val8["id"], 9);

    // Access the same index again
    let val5_again = loader.get(5).unwrap();
    assert_eq!(val5_again["id"], 6);
}

#[test]
fn test_raw_bytes_access() {
    let path = Path::new("tests/fixtures/ndjson/simple.ndjson");
    let loader = LazyJsonFile::open(path).unwrap();

    let raw = loader.raw_slice(0).unwrap();
    let s = String::from_utf8(raw).unwrap();
    assert!(s.contains("\"id\":1"));
    assert!(s.contains("\"name\":\"Alice\""));
}
