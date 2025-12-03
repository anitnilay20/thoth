# Testing Architecture

This document outlines the comprehensive testing strategy for Thoth, ensuring code quality, performance, and reliability.

## Overview

Thoth employs a multi-layered testing approach:

1. **Unit Tests** - Test individual components in isolation
2. **Integration Tests** - Test component interactions and file I/O
3. **Property-Based Tests** - Verify invariants across random inputs
4. **Performance Benchmarks** - Track performance regressions
5. **CI/CD Automation** - Continuous testing and quality checks

## Test Structure

```
thoth/
├── src/
│   └── **/*.rs              # Unit tests (#[cfg(test)] modules)
├── tests/
│   ├── fixtures/            # Test data files
│   │   ├── ndjson/         # NDJSON test files
│   │   ├── json_array/     # JSON array test files
│   │   ├── json_object/    # Single JSON object test files
│   │   └── edge_cases/     # Malformed, large, edge case files
│   ├── common/             # Shared test utilities
│   ├── integration/        # Integration tests
│   └── property/           # Property-based tests
├── benches/                # Criterion benchmarks
│   ├── file_loading.rs
│   ├── search.rs
│   └── parsing.rs
└── docs/
    └── TESTING_ARCHITECTURE.md  # This file
```

## 1. Unit Tests

### Purpose
Test individual functions, structs, and modules in isolation.

### Location
Inside `#[cfg(test)]` modules within source files.

### Coverage Areas

#### File Loaders (`src/file/loaders/`)
- **NdjsonFile**
  - Line boundary indexing
  - CRLF/LF handling
  - Empty files
  - Single-line files
  - Large files (millions of records)

- **JsonArrayFile**
  - Array element indexing
  - Nested arrays/objects
  - Empty arrays
  - Single-element arrays
  - Whitespace handling

- **SingleValueFile**
  - Single object parsing
  - Caching behavior
  - Invalid JSON handling

- **FileLoader Trait**
  - Trait compliance for all loaders
  - `len()`, `get()`, `raw_bytes()` consistency
  - `open()` error handling

#### Search Engine (`src/search/`)
- Substring search correctness
- Case sensitivity
- JSONPath query parsing
- Match highlighting
- Result pagination
- Parallel search correctness

#### Error Handling (`src/error/`)
- Error type conversions
- Error message formatting
- Error propagation

### Example Unit Test

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_ndjson_line_indexing() {
        // Create temporary NDJSON file
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"id":1}}"#).unwrap();
        writeln!(file, r#"{{"id":2}}"#).unwrap();
        writeln!(file, r#"{{"id":3}}"#).unwrap();

        // Load file
        let mut loader = NdjsonFile::open(file.path()).unwrap();

        // Test
        assert_eq!(loader.len(), 3);
        let val = loader.get(1).unwrap();
        assert_eq!(val["id"], 2);
    }
}
```

## 2. Integration Tests

### Purpose
Test complete workflows and component interactions with real file I/O.

### Location
`tests/integration/`

### Test Categories

#### File I/O Integration
- Load various file formats
- Handle large files (>100MB)
- Handle malformed files
- Cross-platform path handling
- File type detection

#### Search Integration
- Search across different file types
- Filter results by JSONPath
- Performance with large datasets
- Cancel long-running searches

#### UI Component Integration (where testable)
- File viewer state management
- Cache behavior with LRU eviction
- Selection persistence

### Example Integration Test

```rust
// tests/integration/file_loading_tests.rs
use thoth::file::loaders::{FileLoader, LazyJsonFile};
use std::path::Path;

#[test]
fn test_load_large_ndjson_file() {
    let path = Path::new("tests/fixtures/ndjson/large_100mb.ndjson");
    let mut loader = LazyJsonFile::open(path).unwrap();

    // Should handle large files without loading all into memory
    assert!(loader.len() > 100_000);

    // Random access should work
    let val = loader.get(50_000).unwrap();
    assert!(val.is_object());
}

#[test]
fn test_detect_file_type() {
    use thoth::file::detect_file_type::sniff_file_type;

    let cases = vec![
        ("tests/fixtures/ndjson/sample.ndjson", DetectedFileType::Ndjson),
        ("tests/fixtures/json_array/array.json", DetectedFileType::JsonArray),
        ("tests/fixtures/json_object/object.json", DetectedFileType::JsonObject),
    ];

    for (path, expected) in cases {
        let detected = sniff_file_type(Path::new(path)).unwrap();
        assert_eq!(detected, expected, "Failed for {}", path);
    }
}
```

## 3. Property-Based Tests

### Purpose
Verify invariants hold across randomly generated inputs using `proptest`.

### Location
`tests/property/`

### Properties to Test

#### JSON Validity
- Any valid JSON can be parsed and re-serialized
- Parsed values match original structure
- No data loss during round-trips

#### File Loader Invariants
- `loader.len()` is always consistent
- `get(i)` succeeds for all `0 <= i < len()`
- `get(i)` fails for `i >= len()`
- `raw_bytes(i)` returns valid JSON when parsed

#### Search Invariants
- Found results always contain the search term
- Result indices are within bounds
- No duplicate results

### Example Property Test

```rust
// tests/property/json_parsing_tests.rs
use proptest::prelude::*;
use serde_json::Value;

proptest! {
    #[test]
    fn test_json_roundtrip(json_str in any::<String>()) {
        // If parsing succeeds, re-serializing should work
        if let Ok(parsed) = serde_json::from_str::<Value>(&json_str) {
            let reserialized = serde_json::to_string(&parsed).unwrap();
            let reparsed: Value = serde_json::from_str(&reserialized).unwrap();
            assert_eq!(parsed, reparsed);
        }
    }

    #[test]
    fn test_file_loader_bounds(num_records in 1usize..1000) {
        // Generate NDJSON with num_records
        let mut temp = NamedTempFile::new().unwrap();
        for i in 0..num_records {
            writeln!(temp, r#"{{"id":{}}}"#, i).unwrap();
        }

        let mut loader = NdjsonFile::open(temp.path()).unwrap();

        // Invariant: len() matches actual records
        assert_eq!(loader.len(), num_records);

        // Invariant: all valid indices succeed
        for i in 0..num_records {
            assert!(loader.get(i).is_ok());
        }

        // Invariant: out-of-bounds fails
        assert!(loader.get(num_records).is_err());
    }
}
```

## 4. Performance Benchmarks

### Purpose
Track performance over time and prevent regressions using `criterion`.

### Location
`benches/`

### Benchmarks

#### File Loading (`benches/file_loading.rs`)
- NDJSON indexing speed (lines/sec)
- JSON array indexing speed
- Memory usage during indexing
- Cold vs warm cache performance

#### Search (`benches/search.rs`)
- Substring search throughput (MB/sec)
- JSONPath query performance
- Parallel vs sequential search
- Result highlighting overhead

#### Parsing (`benches/parsing.rs`)
- JSON parsing speed
- Large object handling
- Deep nesting performance

### Example Benchmark

```rust
// benches/file_loading.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use thoth::file::loaders::{FileLoader, NdjsonFile};
use std::path::Path;

fn bench_ndjson_loading(c: &mut Criterion) {
    let mut group = c.benchmark_group("ndjson_loading");

    for size in [1_000, 10_000, 100_000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let path = format!("benches/fixtures/ndjson_{}.ndjson", size);
            b.iter(|| {
                let loader = NdjsonFile::open(Path::new(&path)).unwrap();
                black_box(loader.len())
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_ndjson_loading);
criterion_main!(benches);
```

## 5. CI/CD Pipeline

### GitHub Actions Workflow

```yaml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        rust: [stable, beta]

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: ${{ matrix.rust }}
          override: true

      - name: Run tests
        run: cargo test --all-features

      - name: Run integration tests
        run: cargo test --test '*'

      - name: Run benchmarks (check only)
        run: cargo bench --no-run

  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin

      - name: Generate coverage
        run: cargo tarpaulin --out Xml --all-features

      - name: Upload to codecov
        uses: codecov/codecov-action@v3
```

## 6. Test Fixtures

### Fixture Organization

```
tests/fixtures/
├── ndjson/
│   ├── simple.ndjson           # 10 basic records
│   ├── nested.ndjson           # Deep nested objects
│   ├── large_1mb.ndjson        # ~10k records
│   ├── large_100mb.ndjson      # ~1M records (gitignored)
│   ├── empty.ndjson            # Empty file
│   ├── single_line.ndjson      # One record
│   └── malformed.ndjson        # Invalid JSON lines
├── json_array/
│   ├── simple.json             # [obj1, obj2, ...]
│   ├── nested.json             # Deeply nested arrays
│   ├── empty.json              # []
│   ├── single_element.json     # [obj]
│   └── malformed.json          # Unclosed arrays
├── json_object/
│   ├── simple.json             # {key: value}
│   ├── nested.json             # Deep nesting
│   ├── empty.json              # {}
│   └── malformed.json          # Invalid JSON
└── edge_cases/
    ├── unicode.json            # Unicode characters
    ├── escaped.json            # Escaped strings
    ├── numbers.json            # Large numbers, floats
    └── mixed_types.json        # All JSON types
```

### Fixture Generation

Create fixtures programmatically for consistent testing:

```rust
// tests/common/fixture_generator.rs
pub fn generate_ndjson(path: &Path, num_records: usize) {
    let mut file = File::create(path).unwrap();
    for i in 0..num_records {
        writeln!(file, r#"{{"id":{},"data":"record_{}}}"#, i, i).unwrap();
    }
}
```

## 7. Running Tests

### Commands

```bash
# Run all tests
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test '*'

# Run with coverage
cargo tarpaulin --out Html

# Run benchmarks
cargo bench

# Run specific test
cargo test test_ndjson_loading

# Run tests in parallel
cargo test -- --test-threads=8

# Show test output
cargo test -- --nocapture
```

## 8. Test Coverage Goals

### Target Coverage
- **Overall**: >80%
- **Core modules** (file loaders, search): >90%
- **UI components**: Best effort (limited by egui testability)

### Measurement
Use `cargo-tarpaulin` for coverage reporting:

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --output-dir coverage
```

## 9. Testing Best Practices

### Guidelines

1. **Test Naming**: Use descriptive names
   - ✅ `test_ndjson_handles_crlf_line_endings`
   - ❌ `test1`

2. **Arrange-Act-Assert**: Clear test structure
   ```rust
   // Arrange
   let input = create_test_data();

   // Act
   let result = function_under_test(input);

   // Assert
   assert_eq!(result, expected);
   ```

3. **One Assertion Per Test**: Focus on single behavior
4. **Use Helpers**: Share setup code in test utilities
5. **Clean Up**: Use `tempfile` for temporary files
6. **Fast Tests**: Keep unit tests <1ms, integration tests <100ms
7. **Deterministic**: No random behavior (unless using proptest with seeds)

### Anti-Patterns to Avoid

- ❌ Tests that depend on execution order
- ❌ Tests that require manual setup
- ❌ Tests that access network/external services
- ❌ Tests with hardcoded absolute paths
- ❌ Flaky tests that pass/fail randomly

## 10. Continuous Improvement

### Monitoring
- Track test execution time
- Monitor flaky tests
- Review coverage reports weekly
- Update benchmarks for regressions

### When to Add Tests
- **Always**: When fixing bugs (regression test)
- **Always**: When adding features
- **Consider**: When refactoring (to ensure behavior unchanged)

## Summary

This testing architecture ensures Thoth maintains high quality through:
- ✅ Comprehensive unit test coverage
- ✅ Real-world integration testing
- ✅ Property-based invariant verification
- ✅ Performance regression tracking
- ✅ Automated CI/CD quality gates
- ✅ Cross-platform validation

Following this strategy will catch bugs early, enable confident refactoring, and maintain performance standards as the codebase grows.
