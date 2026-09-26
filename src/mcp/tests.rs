//! Tests for the MCP server's state — the handle table and what a tool is
//! allowed to run through it.
//!
//! The tools themselves are thin: they look a handle up, call one method on
//! the open file and wrap the answer. What is worth pinning is the part with
//! rules — handles being distinct and independent, the format a file reports,
//! the bound on how much a query may return, and the statements a client is
//! allowed to send at all.

#![cfg(test)]

use std::io::Write;
use std::path::PathBuf;

use tempfile::NamedTempFile;

use crate::mcp::state::ServerState;

/// A file with a given extension and contents.
fn file(suffix: &str, contents: &str) -> NamedTempFile {
    let mut f = tempfile::Builder::new()
        .suffix(suffix)
        .tempfile()
        .expect("temp file");
    write!(f, "{contents}").unwrap();
    f.flush().unwrap();
    f
}

fn ndjson(lines: &[&str]) -> NamedTempFile {
    file(".ndjson", &format!("{}\n", lines.join("\n")))
}

// ── The handle table ─────────────────────────────────────────────────────────

#[test]
fn a_file_opens_reports_itself_and_closes_once() {
    let f = ndjson(&[
        r#"{"name":"alice","age":30}"#,
        r#"{"name":"bob","age":25}"#,
        r#"{"name":"carol","age":35}"#,
    ]);

    let state = ServerState::new();
    let (handle, info) = state.open_file(f.path()).expect("opened");

    assert!(!handle.is_empty());
    assert_eq!(info.file_type, "ndjson");
    assert_eq!(info.record_count, 3);
    assert!(
        !info.alias.is_empty(),
        "SQL needs a name to reference it by"
    );

    // Looking it up again says the same thing.
    let again = state.file_info(&handle).expect("still open");
    assert_eq!(again.record_count, 3);
    assert_eq!(again.file_type, "ndjson");

    assert!(state.close_file(&handle));
    // Closing twice is not an error, but it is not a close either — a client
    // that retries must not be told it closed something a second time.
    assert!(!state.close_file(&handle));
    assert!(state.file_info(&handle).is_none());
}

#[test]
fn opening_a_file_twice_yields_two_independent_handles() {
    let f = ndjson(&[r#"{"a":1}"#]);
    let state = ServerState::new();

    let (first, _) = state.open_file(f.path()).unwrap();
    let (second, _) = state.open_file(f.path()).unwrap();

    assert_ne!(first, second, "one handle per open, not per path");
    // And closing one leaves the other usable.
    assert!(state.close_file(&first));
    assert!(state.file_info(&second).is_some());
}

#[test]
fn files_opened_together_stay_apart() {
    let a = ndjson(&[r#"{"a":1}"#, r#"{"a":2}"#]);
    let b = file(".csv", "b\n1\n2\n3\n");

    let state = ServerState::new();
    let (ha, info_a) = state.open_file(a.path()).unwrap();
    let (hb, info_b) = state.open_file(b.path()).unwrap();

    assert_eq!(info_a.record_count, 2);
    assert_eq!(info_b.record_count, 3);
    assert_eq!(state.list_handles().len(), 2);

    state.close_file(&ha);
    assert!(state.file_info(&ha).is_none());
    assert!(state.file_info(&hb).is_some());
}

#[test]
fn a_missing_file_fails_rather_than_opening_empty() {
    let state = ServerState::new();
    assert!(
        state
            .open_file(&PathBuf::from("/nonexistent/path/to/file.json"))
            .is_err()
    );
}

#[test]
fn an_unknown_handle_answers_nothing() {
    let state = ServerState::new();
    assert!(state.with_file("nonexistent", |_| 42).is_none());
    assert!(state.file_info("nonexistent").is_none());
}

#[test]
fn many_readers_share_one_open_file() {
    let f = ndjson(&[r#"{"name":"alice"}"#, r#"{"name":"bob"}"#]);
    let state = ServerState::new();
    let (handle, _) = state.open_file(f.path()).unwrap();

    let readers: Vec<_> = (0..4)
        .map(|_| {
            let state = state.clone();
            let handle = handle.clone();
            std::thread::spawn(move || {
                for _ in 0..10 {
                    let info = state.file_info(&handle).expect("still open");
                    assert_eq!(info.record_count, 2);
                }
            })
        })
        .collect();
    for reader in readers {
        reader.join().unwrap();
    }
}

// ── What a file reports itself as ────────────────────────────────────────────

#[test]
fn a_file_reports_the_format_it_actually_is() {
    // Reported through `FileKind`, half of these came back as "json": that
    // enum does not carry the distinction, and its `Ndjson` arm was
    // unreachable from a `FileType`.
    let state = ServerState::new();
    for (suffix, contents, expected) in [
        (".ndjson", "{\"a\":1}\n{\"a\":2}\n", "ndjson"),
        (".jsonl", "{\"a\":1}\n", "ndjson"),
        (".json", "[{\"a\":1}]", "json"),
        (".csv", "a\n1\n", "csv"),
    ] {
        let f = file(suffix, contents);
        let (_, info) = state.open_file(f.path()).expect(suffix);
        assert_eq!(info.file_type, expected, "{suffix} reported as its format");
    }
}

// ── Querying ─────────────────────────────────────────────────────────────────

#[test]
fn a_query_returns_at_most_what_was_asked_for() {
    let rows: Vec<String> = (0..500).map(|i| format!(r#"{{"n":{i}}}"#)).collect();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    let f = ndjson(&refs);

    let state = ServerState::new();
    let (handle, info) = state.open_file(f.path()).unwrap();

    let alias = info.alias;
    let got = state
        .with_file(&handle, |file| {
            file.query(&format!("SELECT * FROM {alias} ORDER BY n"), 10)
        })
        .expect("handle resolves")
        .expect("query runs");

    assert_eq!(got.len(), 10, "bounded by the caller, not by the file");
    assert_eq!(got[0]["n"], serde_json::json!(0));
    assert_eq!(got[9]["n"], serde_json::json!(9));
}

#[test]
fn a_query_that_reads_nothing_returns_nothing_rather_than_failing() {
    let f = ndjson(&[r#"{"n":1}"#]);
    let state = ServerState::new();
    let (handle, info) = state.open_file(f.path()).unwrap();
    let alias = info.alias;

    let got = state
        .with_file(&handle, |file| {
            file.query(&format!("SELECT * FROM {alias} WHERE n > 99"), 10)
        })
        .unwrap()
        .expect("an empty result is a result");
    assert!(got.is_empty());
}

#[test]
fn only_read_only_statements_reach_the_engine() {
    let f = ndjson(&[r#"{"n":1}"#]);
    let state = ServerState::new();
    let (handle, info) = state.open_file(f.path()).unwrap();
    let alias = info.alias;

    let run = |sql: String| {
        state
            .with_file(&handle, |file| file.query(&sql, 10))
            .expect("handle resolves")
    };

    // The engine a tool runs against has the user's file system and network
    // reach. An MCP client is not the user.
    for hostile in [
        format!("COPY (SELECT * FROM {alias}) TO '/tmp/thoth-mcp-should-not-exist.csv'"),
        "ATTACH 'http://example.invalid/x.db' AS remote".to_string(),
        "INSTALL httpfs".to_string(),
        "CREATE TABLE pwned (x INT)".to_string(),
        format!("SELECT * FROM {alias}; CREATE TABLE pwned (x INT)"),
        // A `;` inside a literal is not a second statement, but a comment
        // must not be able to hide one either.
        format!("SELECT * FROM {alias} -- ;\n; DROP TABLE {alias}"),
    ] {
        assert!(
            run(hostile.clone()).is_err(),
            "{hostile:?} was allowed through"
        );
    }
    assert!(
        !std::path::Path::new("/tmp/thoth-mcp-should-not-exist.csv").exists(),
        "a refused COPY still wrote a file"
    );

    // And the reads a client is actually for still work, including a trailing
    // semicolon and a literal that happens to contain one.
    for fine in [
        format!("SELECT * FROM {alias}"),
        format!("SELECT * FROM {alias};"),
        format!("WITH t AS (SELECT * FROM {alias}) SELECT * FROM t"),
        format!("DESCRIBE SELECT * FROM {alias}"),
        format!("SELECT 'a;b' AS s FROM {alias}"),
    ] {
        assert!(run(fine.clone()).is_ok(), "{fine:?} was refused");
    }
}
