use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use serde_json::Value;
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;
use thoth::file::detect_file_type::sniff_file_type;

/// Generate JSON objects of varying complexity
fn generate_simple_json() -> String {
    r#"{"id":1,"name":"test","value":100}"#.to_string()
}

fn generate_nested_json() -> String {
    r#"{"id":1,"user":{"name":"test","email":"test@example.com"},"metadata":{"created":"2024-01-01","tags":["a","b","c"]},"value":100}"#.to_string()
}

fn generate_deeply_nested_json() -> String {
    r#"{"level1":{"level2":{"level3":{"level4":{"level5":{"level6":{"level7":{"level8":{"level9":{"level10":{"value":"deep"}}}}}}}}}}}"#.to_string()
}

fn generate_large_array_json(size: usize) -> String {
    let mut json = String::from("[");
    for i in 0..size {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(r#"{{"id":{},"value":{}}}"#, i, i * 100));
    }
    json.push(']');
    json
}

fn generate_wide_object_json(num_fields: usize) -> String {
    let mut json = String::from("{");
    for i in 0..num_fields {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(r#""field{}":"value{}""#, i, i));
    }
    json.push('}');
    json
}

/// Benchmark: JSON parsing with different complexities
fn bench_json_parsing_complexity(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_parsing_complexity");

    let simple = generate_simple_json();
    let nested = generate_nested_json();
    let deeply_nested = generate_deeply_nested_json();

    group.bench_function("simple_object", |b| {
        b.iter(|| {
            let value: Value = serde_json::from_str(black_box(&simple)).unwrap();
            black_box(value)
        });
    });

    group.bench_function("nested_object", |b| {
        b.iter(|| {
            let value: Value = serde_json::from_str(black_box(&nested)).unwrap();
            black_box(value)
        });
    });

    group.bench_function("deeply_nested", |b| {
        b.iter(|| {
            let value: Value = serde_json::from_str(black_box(&deeply_nested)).unwrap();
            black_box(value)
        });
    });

    group.finish();
}

/// Benchmark: JSON parsing with different array sizes
fn bench_json_array_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_array_sizes");

    for size in [10, 100, 1000].iter() {
        let json = generate_large_array_json(*size);
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let value: Value = serde_json::from_str(black_box(&json)).unwrap();
                black_box(value)
            });
        });
    }

    group.finish();
}

/// Benchmark: JSON parsing with different object widths (number of fields)
fn bench_json_object_width(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_object_width");

    for num_fields in [10, 50, 200].iter() {
        let json = generate_wide_object_json(*num_fields);
        group.throughput(Throughput::Elements(*num_fields as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_fields),
            num_fields,
            |b, _| {
                b.iter(|| {
                    let value: Value = serde_json::from_str(black_box(&json)).unwrap();
                    black_box(value)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: File type detection
fn bench_file_type_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_type_detection");

    let temp_dir = TempDir::new().unwrap();

    // Create NDJSON file
    let ndjson_path = temp_dir.path().join("test.ndjson");
    {
        let mut file = File::create(&ndjson_path).unwrap();
        for i in 0..100 {
            writeln!(file, r#"{{"id":{},"name":"Record {}"}}"#, i, i).unwrap();
        }
    }

    // Create JSON array file
    let json_array_path = temp_dir.path().join("test_array.json");
    {
        let mut file = File::create(&json_array_path).unwrap();
        write!(file, "[").unwrap();
        for i in 0..100 {
            if i > 0 {
                write!(file, ",").unwrap();
            }
            write!(file, r#"{{"id":{},"name":"Element {}"}}"#, i, i).unwrap();
        }
        write!(file, "]").unwrap();
    }

    // Create JSON object file
    let json_object_path = temp_dir.path().join("test_object.json");
    {
        let mut file = File::create(&json_object_path).unwrap();
        write!(file, r#"{{"id":1,"name":"Single Object"}}"#).unwrap();
    }

    group.bench_function("detect_ndjson", |b| {
        b.iter(|| black_box(sniff_file_type(black_box(&ndjson_path)).unwrap()));
    });

    group.bench_function("detect_json_array", |b| {
        b.iter(|| black_box(sniff_file_type(black_box(&json_array_path)).unwrap()));
    });

    group.bench_function("detect_json_object", |b| {
        b.iter(|| black_box(sniff_file_type(black_box(&json_object_path)).unwrap()));
    });

    group.finish();
}

/// Benchmark: JSON serialization (parsing in reverse)
fn bench_json_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_serialization");

    let simple: Value = serde_json::from_str(&generate_simple_json()).unwrap();
    let nested: Value = serde_json::from_str(&generate_nested_json()).unwrap();
    let array: Value = serde_json::from_str(&generate_large_array_json(100)).unwrap();

    group.bench_function("simple_object", |b| {
        b.iter(|| black_box(serde_json::to_string(black_box(&simple)).unwrap()));
    });

    group.bench_function("nested_object", |b| {
        b.iter(|| black_box(serde_json::to_string(black_box(&nested)).unwrap()));
    });

    group.bench_function("array_100_elements", |b| {
        b.iter(|| black_box(serde_json::to_string(black_box(&array)).unwrap()));
    });

    group.finish();
}

/// Benchmark: Parsing from bytes vs string
fn bench_parse_bytes_vs_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_bytes_vs_string");

    let json_str = generate_nested_json();
    let json_bytes = json_str.as_bytes();

    group.bench_function("from_string", |b| {
        b.iter(|| {
            let value: Value = serde_json::from_str(black_box(&json_str)).unwrap();
            black_box(value)
        });
    });

    group.bench_function("from_bytes", |b| {
        b.iter(|| {
            let value: Value = serde_json::from_slice(black_box(json_bytes)).unwrap();
            black_box(value)
        });
    });

    group.finish();
}

/// Benchmark: Whitespace handling in JSON
fn bench_json_whitespace(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_whitespace");

    let compact = r#"{"id":1,"name":"test","value":100}"#;
    let pretty = r#"{
  "id": 1,
  "name": "test",
  "value": 100
}"#;
    let extra_whitespace = r#"{  "id"  :  1  ,  "name"  :  "test"  ,  "value"  :  100  }"#;

    group.bench_function("compact", |b| {
        b.iter(|| {
            let value: Value = serde_json::from_str(black_box(compact)).unwrap();
            black_box(value)
        });
    });

    group.bench_function("pretty", |b| {
        b.iter(|| {
            let value: Value = serde_json::from_str(black_box(pretty)).unwrap();
            black_box(value)
        });
    });

    group.bench_function("extra_whitespace", |b| {
        b.iter(|| {
            let value: Value = serde_json::from_str(black_box(extra_whitespace)).unwrap();
            black_box(value)
        });
    });

    group.finish();
}

/// Benchmark: Value access patterns
fn bench_value_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("value_access");

    let json_str = generate_nested_json();
    let value: Value = serde_json::from_str(&json_str).unwrap();

    group.bench_function("top_level_field", |b| {
        b.iter(|| black_box(value.get("id")));
    });

    group.bench_function("nested_field", |b| {
        b.iter(|| black_box(value.get("user").and_then(|u| u.get("email"))));
    });

    group.bench_function("array_index", |b| {
        b.iter(|| {
            black_box(
                value
                    .get("metadata")
                    .and_then(|m| m.get("tags"))
                    .and_then(|t| t.get(0)),
            )
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_json_parsing_complexity,
    bench_json_array_sizes,
    bench_json_object_width,
    bench_file_type_detection,
    bench_json_serialization,
    bench_parse_bytes_vs_string,
    bench_json_whitespace,
    bench_value_access
);
criterion_main!(benches);
