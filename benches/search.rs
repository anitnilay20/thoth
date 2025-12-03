use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::fs::File;
use std::io::Write;
use tempfile::TempDir;
use thoth::file::loaders::FileType;
use thoth::search::{QueryMode, Search};

/// Generate a temporary NDJSON file with the specified number of records
fn create_ndjson_file(temp_dir: &TempDir, num_records: usize) -> std::path::PathBuf {
    let file_path = temp_dir.path().join("search_benchmark.ndjson");
    let mut file = File::create(&file_path).unwrap();

    for i in 0..num_records {
        writeln!(
            file,
            r#"{{"id":{},"name":"Record {}","email":"user{}@example.com","status":"active","value":{},"nested":{{"field":"data","tag":"tag{}"}},"description":"This is a test record number {} with some searchable text"}}"#,
            i, i, i, i * 100, i % 10, i
        )
        .unwrap();
    }

    file_path
}

/// Create a file with some matches at different densities
fn create_file_with_match_density(
    temp_dir: &TempDir,
    num_records: usize,
    match_every_n: usize,
) -> std::path::PathBuf {
    let file_path = temp_dir.path().join("density_benchmark.ndjson");
    let mut file = File::create(&file_path).unwrap();

    for i in 0..num_records {
        let content = if i % match_every_n == 0 {
            format!(
                r#"{{"id":{},"name":"MATCH Record {}","value":{},"tag":"special"}}"#,
                i,
                i,
                i * 100
            )
        } else {
            format!(
                r#"{{"id":{},"name":"Record {}","value":{}}}"#,
                i,
                i,
                i * 100
            )
        };
        writeln!(file, "{}", content).unwrap();
    }

    file_path
}

/// Benchmark: Text search with different file sizes
fn bench_text_search_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_search_scaling");

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let temp_dir = TempDir::new().unwrap();
            let file_path = create_ndjson_file(&temp_dir, size);

            b.iter(|| {
                let mut search = Search {
                    query: black_box("user".to_string()),
                    match_case: false,
                    query_mode: QueryMode::Text,
                    ..Search::default()
                };
                search.start_scanning_internal(&Some(file_path.clone()), &FileType::Ndjson);
                black_box(search.results.hits().len())
            });
        });
    }

    group.finish();
}

/// Benchmark: Case-sensitive vs case-insensitive search
fn bench_case_sensitivity(c: &mut Criterion) {
    let mut group = c.benchmark_group("case_sensitivity");

    let size = 1000;
    let temp_dir = TempDir::new().unwrap();
    let file_path = create_ndjson_file(&temp_dir, size);

    group.throughput(Throughput::Elements(size as u64));

    group.bench_function("case_sensitive", |b| {
        b.iter(|| {
            let mut search = Search {
                query: black_box("Record".to_string()),
                match_case: true,
                query_mode: QueryMode::Text,
                ..Search::default()
            };
            search.start_scanning_internal(&Some(file_path.clone()), &FileType::Ndjson);
            black_box(search.results.hits().len())
        });
    });

    group.bench_function("case_insensitive", |b| {
        b.iter(|| {
            let mut search = Search {
                query: black_box("record".to_string()),
                match_case: false,
                query_mode: QueryMode::Text,
                ..Search::default()
            };
            search.start_scanning_internal(&Some(file_path.clone()), &FileType::Ndjson);
            black_box(search.results.hits().len())
        });
    });

    group.finish();
}

/// Benchmark: Search with different match densities
fn bench_match_density(c: &mut Criterion) {
    let mut group = c.benchmark_group("match_density");

    let size = 1000;

    for density in [2, 10, 100].iter() {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("match_every_n", density),
            density,
            |b, &density| {
                let temp_dir = TempDir::new().unwrap();
                let file_path = create_file_with_match_density(&temp_dir, size, density);

                b.iter(|| {
                    let mut search = Search {
                        query: black_box("MATCH".to_string()),
                        match_case: false,
                        query_mode: QueryMode::Text,
                        ..Search::default()
                    };
                    search.start_scanning_internal(&Some(file_path.clone()), &FileType::Ndjson);
                    black_box(search.results.hits().len())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: JSONPath queries vs text search
fn bench_jsonpath_vs_text(c: &mut Criterion) {
    let mut group = c.benchmark_group("jsonpath_vs_text");

    let size = 1000;
    let temp_dir = TempDir::new().unwrap();
    let file_path = create_ndjson_file(&temp_dir, size);

    group.throughput(Throughput::Elements(size as u64));

    group.bench_function("text_search", |b| {
        b.iter(|| {
            let mut search = Search {
                query: black_box("example.com".to_string()),
                match_case: false,
                query_mode: QueryMode::Text,
                ..Search::default()
            };
            search.start_scanning_internal(&Some(file_path.clone()), &FileType::Ndjson);
            black_box(search.results.hits().len())
        });
    });

    group.bench_function("jsonpath_simple", |b| {
        b.iter(|| {
            let mut search = Search {
                query: black_box("$.email".to_string()),
                match_case: false,
                query_mode: QueryMode::JsonPath,
                ..Search::default()
            };
            search.start_scanning_internal(&Some(file_path.clone()), &FileType::Ndjson);
            black_box(search.results.hits().len())
        });
    });

    group.bench_function("jsonpath_recursive", |b| {
        b.iter(|| {
            let mut search = Search {
                query: black_box("$..field".to_string()),
                match_case: false,
                query_mode: QueryMode::JsonPath,
                ..Search::default()
            };
            search.start_scanning_internal(&Some(file_path.clone()), &FileType::Ndjson);
            black_box(search.results.hits().len())
        });
    });

    group.finish();
}

/// Benchmark: Different query lengths
fn bench_query_length(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_length");

    let size = 1000;
    let temp_dir = TempDir::new().unwrap();
    let file_path = create_ndjson_file(&temp_dir, size);

    group.throughput(Throughput::Elements(size as u64));

    let queries = vec![
        ("short", "id"),
        ("medium", "Record"),
        ("long", "searchable text"),
        ("very_long", "This is a test record number"),
    ];

    for (name, query) in queries {
        group.bench_function(name, |b| {
            b.iter(|| {
                let mut search = Search {
                    query: black_box(query.to_string()),
                    match_case: false,
                    query_mode: QueryMode::Text,
                    ..Search::default()
                };
                search.start_scanning_internal(&Some(file_path.clone()), &FileType::Ndjson);
                black_box(search.results.hits().len())
            });
        });
    }

    group.finish();
}

/// Benchmark: Field matching overhead
fn bench_field_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("field_matching");

    let size = 1000;
    let temp_dir = TempDir::new().unwrap();
    let file_path = create_ndjson_file(&temp_dir, size);

    group.throughput(Throughput::Elements(size as u64));

    // Search for term that appears in structured fields
    group.bench_function("field_match", |b| {
        b.iter(|| {
            let mut search = Search {
                query: black_box("active".to_string()),
                match_case: false,
                query_mode: QueryMode::Text,
                ..Search::default()
            };
            search.start_scanning_internal(&Some(file_path.clone()), &FileType::Ndjson);
            black_box(search.results.hits().len())
        });
    });

    // Search for term in nested fields
    group.bench_function("nested_field_match", |b| {
        b.iter(|| {
            let mut search = Search {
                query: black_box("data".to_string()),
                match_case: false,
                query_mode: QueryMode::Text,
                ..Search::default()
            };
            search.start_scanning_internal(&Some(file_path.clone()), &FileType::Ndjson);
            black_box(search.results.hits().len())
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_text_search_scaling,
    bench_case_sensitivity,
    bench_match_density,
    bench_jsonpath_vs_text,
    bench_query_length,
    bench_field_matching
);
criterion_main!(benches);
