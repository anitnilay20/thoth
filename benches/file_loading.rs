use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;
use thoth::file::loaders::{FileLoader, load_file_auto};

/// Generate a temporary NDJSON file with the specified number of records
fn create_ndjson_file(temp_dir: &TempDir, num_records: usize) -> std::path::PathBuf {
    let file_path = temp_dir.path().join("benchmark.ndjson");
    let mut file = File::create(&file_path).unwrap();

    for i in 0..num_records {
        writeln!(
            file,
            r#"{{"id":{},"name":"Record {}","value":{},"nested":{{"field":"data"}}}}"#,
            i,
            i,
            i * 100
        )
        .unwrap();
    }

    file_path
}

/// Generate a temporary JSON array file with the specified number of elements
fn create_json_array_file(temp_dir: &TempDir, num_elements: usize) -> std::path::PathBuf {
    let file_path = temp_dir.path().join("benchmark.json");
    let mut file = File::create(&file_path).unwrap();

    write!(file, "[").unwrap();
    for i in 0..num_elements {
        if i > 0 {
            write!(file, ",").unwrap();
        }
        write!(
            file,
            r#"{{"id":{},"name":"Element {}","value":{},"nested":{{"field":"data"}}}}"#,
            i,
            i,
            i * 100
        )
        .unwrap();
    }
    write!(file, "]").unwrap();

    file_path
}

/// Benchmark: Loading different sized NDJSON files
fn bench_ndjson_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("ndjson_loading");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let temp_dir = TempDir::new().unwrap();
            let file_path = create_ndjson_file(&temp_dir, size);

            b.iter(|| {
                let (_detected, file) = load_file_auto(black_box(&file_path)).unwrap();
                black_box(file.len())
            });
        });
    }

    group.finish();
}

/// Benchmark: Loading different sized JSON array files
fn bench_json_array_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_array_loading");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let temp_dir = TempDir::new().unwrap();
            let file_path = create_json_array_file(&temp_dir, size);

            b.iter(|| {
                let (_detected, file) = load_file_auto(black_box(&file_path)).unwrap();
                black_box(file.len())
            });
        });
    }

    group.finish();
}

/// Benchmark: Sequential access patterns (reading all records)
fn bench_sequential_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_access");

    for size in [100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let temp_dir = TempDir::new().unwrap();
            let file_path = create_ndjson_file(&temp_dir, size);

            b.iter(|| {
                let (_detected, mut file) = load_file_auto(&file_path).unwrap();
                let len = file.len();
                for i in 0..len {
                    black_box(file.get(black_box(i)).unwrap());
                }
            });
        });
    }

    group.finish();
}

/// Benchmark: Random access patterns
fn bench_random_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("random_access");

    let size = 1000;
    let temp_dir = TempDir::new().unwrap();
    let file_path = create_ndjson_file(&temp_dir, size);

    group.throughput(Throughput::Elements(100));
    group.bench_function("random_1000_records", |b| {
        b.iter(|| {
            let (_detected, mut file) = load_file_auto(&file_path).unwrap();
            // Access 100 random positions
            for i in (0..100).map(|x| (x * 13) % size) {
                black_box(file.get(black_box(i)).unwrap());
            }
        });
    });

    group.finish();
}

/// Benchmark: Getting raw bytes vs parsed JSON
fn bench_raw_bytes_vs_parsed(c: &mut Criterion) {
    let mut group = c.benchmark_group("raw_bytes_vs_parsed");

    let size = 1000;
    let temp_dir = TempDir::new().unwrap();
    let file_path = create_ndjson_file(&temp_dir, size);

    group.bench_function("raw_bytes", |b| {
        b.iter(|| {
            let (_detected, file) = load_file_auto(&file_path).unwrap();
            black_box(file.raw_bytes(black_box(500)).unwrap())
        });
    });

    group.bench_function("parsed_json", |b| {
        b.iter(|| {
            let (_detected, mut file) = load_file_auto(&file_path).unwrap();
            black_box(file.get(black_box(500)).unwrap())
        });
    });

    group.finish();
}

/// Benchmark: File type detection overhead
fn bench_file_type_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_type_detection");

    let temp_dir = TempDir::new().unwrap();
    let ndjson_path = create_ndjson_file(&temp_dir, 1000);
    let json_array_path = create_json_array_file(&temp_dir, 1000);

    group.bench_function("ndjson_detection", |b| {
        b.iter(|| black_box(load_file_auto(black_box(&ndjson_path)).unwrap()));
    });

    group.bench_function("json_array_detection", |b| {
        b.iter(|| black_box(load_file_auto(black_box(&json_array_path)).unwrap()));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_ndjson_loading,
    bench_json_array_loading,
    bench_sequential_access,
    bench_random_access,
    bench_raw_bytes_vs_parsed,
    bench_file_type_detection
);
criterion_main!(benches);
