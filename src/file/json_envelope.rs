//! Finding the collections inside a JSON document that is one big object.
//!
//! A large JSON export is rarely one dataset. It is usually an envelope — some
//! metadata and several named arrays of records:
//!
//! ```json
//! { "metadata": {...}, "users": [...], "transactions": [...], "logs": [...] }
//! ```
//!
//! DuckDB cannot reach inside that. One object is one row, so it must parse the
//! whole document to read any part of it, which is both slow and useless: the
//! result is a single row whose columns are hundred-megabyte blobs. There is
//! nothing to join or aggregate.
//!
//! The contents, though, are perfectly tabular. `users` is a table. So this
//! scans the document once at the byte level — tracking nesting and string
//! state, never materializing a value — and records where each top-level key's
//! value begins and ends. With that, a single collection can be handed to
//! DuckDB on its own, and `users JOIN transactions` becomes ordinary SQL.
//!
//! The scan is I/O bound and allocates nothing per record: roughly a second per
//! gigabyte.

use std::fs::File;
use std::io::{BufReader, Read};
use std::ops::ControlFlow;
use std::path::Path;

use crate::error::{Result, ThothError};

/// Bytes read per scan iteration.
const SCAN_CHUNK: usize = 1 << 20;

/// What a top-level value is, decided by its first byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// An array — the shape worth handing to DuckDB as a table.
    Array,
    Object,
    Scalar,
}

/// A top-level key and the byte range of its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    pub name: String,
    pub kind: ValueKind,
    /// Byte offset of the value's first byte.
    pub start: u64,
    /// Byte offset one past the value's last byte.
    pub end: u64,
}

impl ValueKind {
    /// Stable name, for persisting a layout.
    pub fn as_str(self) -> &'static str {
        match self {
            ValueKind::Array => "array",
            ValueKind::Object => "object",
            ValueKind::Scalar => "scalar",
        }
    }

    /// Parse a persisted name; anything unrecognised reads as a scalar, which
    /// is the harmless case — it is listed but not queried.
    pub fn parse(text: &str) -> Self {
        match text {
            "array" => ValueKind::Array,
            "object" => ValueKind::Object,
            _ => ValueKind::Scalar,
        }
    }
}

impl Collection {
    /// Size of the value in bytes.
    pub fn len(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether this is worth exposing to DuckDB as a table.
    ///
    /// An empty array spans exactly `[]`, so two bytes is the floor for one
    /// holding anything. A padded `[ ]` slips through and yields a table with
    /// no rows, which is harmless.
    pub fn is_queryable(&self) -> bool {
        self.kind == ValueKind::Array && self.len() > 2
    }
}

/// The collections found in a document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JsonEnvelope {
    pub collections: Vec<Collection>,
}

impl JsonEnvelope {
    /// Scan `path`, recording each top-level key's value range.
    ///
    /// Returns `Ok(None)` if the document is not a single top-level object —
    /// an array or scalar has no envelope, and DuckDB reads those natively.
    pub fn scan(path: &Path) -> Result<Option<Self>> {
        Self::scan_observed(path, |_| ControlFlow::Continue(())).map(|outcome| outcome.flatten())
    }

    /// Scan, reporting bytes consumed and honouring a request to stop.
    ///
    /// The outer `Option` is cancellation; the inner one is "no envelope here".
    pub fn scan_observed(
        path: &Path,
        mut on_progress: impl FnMut(u64) -> ControlFlow<()>,
    ) -> Result<Option<Option<Self>>> {
        let file = File::open(path).map_err(|e| ThothError::FileReadError {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;
        let mut reader = BufReader::new(file);
        let mut buf = vec![0u8; SCAN_CHUNK];

        let mut scanner = Scanner::default();
        let mut offset: u64 = 0;

        loop {
            let read = reader.read(&mut buf).map_err(|e| ThothError::FileReadError {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })?;
            if read == 0 {
                break;
            }
            if scanner.feed(&buf[..read], offset).is_break() {
                // Not an object at the root; nothing to do here.
                return Ok(Some(None));
            }
            offset += read as u64;
            scanner.at_eof = offset;
            if on_progress(offset).is_break() {
                return Ok(None);
            }
        }

        Ok(Some(scanner.finish()))
    }

    /// Collections worth querying — non-empty arrays.
    pub fn queryable(&self) -> impl Iterator<Item = &Collection> {
        self.collections.iter().filter(|c| c.is_queryable())
    }

    /// A collection by name.
    pub fn get(&self, name: &str) -> Option<&Collection> {
        self.collections.iter().find(|c| c.name == name)
    }
}

/// The next byte inside a string that can end it or escape the one after.
fn next_in_string(hay: &[u8]) -> Option<usize> {
    memchr::memchr2(b'"', b'\\', hay)
}

/// Where the scanner is in the document.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
enum State {
    /// Before the root `{`.
    #[default]
    BeforeRoot,
    /// Inside the root object, expecting a key or `}`.
    ExpectKey,
    /// Reading a key string.
    InKey,
    /// After a key, expecting `:`.
    ExpectColon,
    /// After `:`, expecting the value's first byte.
    ExpectValue,
    /// Inside a value, skipping to its end.
    InValue,
    /// Past the root object's close.
    Done,
}

/// A byte-level JSON structural scanner.
///
/// Deliberately not a parser: it never builds a value, only tracks enough state
/// to know where each top-level value starts and stops. That is what lets it
/// run over a multi-gigabyte document in one pass with no allocation per
/// record.
#[derive(Debug, Default)]
struct Scanner {
    state: State,
    /// Nesting depth within the current value.
    depth: i32,
    /// Inside a string literal.
    in_string: bool,
    /// The previous byte was a backslash inside a string.
    escaped: bool,
    key: String,
    value_start: u64,
    value_first: u8,
    /// Bytes consumed, so a value left open by a truncated document can still
    /// be closed at the end of what exists.
    at_eof: u64,
    found: Vec<Collection>,
}

impl Scanner {
    /// Consume a chunk. `base` is the chunk's offset in the file.
    ///
    /// Returns `Break` if the document's root is not an object.
    fn feed(&mut self, chunk: &[u8], base: u64) -> ControlFlow<()> {
        let mut i = 0;
        while i < chunk.len() {
            // Inside a string, skip straight to whatever can end or escape it.
            // String contents are the one place a JSON document has long runs
            // of bytes that cannot affect structure; punctuation elsewhere is
            // dense enough that a search costs more setup than it saves.
            if self.state == State::InValue && self.in_string && !self.escaped {
                match next_in_string(&chunk[i..]) {
                    Some(0) => {}
                    Some(n) => {
                        i += n;
                        continue;
                    }
                    None => break,
                }
            }

            let byte = chunk[i];
            let at = base + i as u64;
            match self.state {
                State::BeforeRoot => {
                    if byte.is_ascii_whitespace() {
                        i += 1;
                        continue;
                    }
                    if byte != b'{' {
                        return ControlFlow::Break(());
                    }
                    self.state = State::ExpectKey;
                }
                State::ExpectKey => match byte {
                    b'"' => {
                        self.key.clear();
                        self.in_string = true;
                        self.escaped = false;
                        self.state = State::InKey;
                    }
                    b'}' => self.state = State::Done,
                    _ => {} // whitespace and commas
                },
                State::InKey => {
                    if self.escaped {
                        self.key.push(byte as char);
                        self.escaped = false;
                    } else if byte == b'\\' {
                        self.escaped = true;
                    } else if byte == b'"' {
                        self.in_string = false;
                        self.state = State::ExpectColon;
                    } else {
                        self.key.push(byte as char);
                    }
                }
                State::ExpectColon => {
                    if byte == b':' {
                        self.state = State::ExpectValue;
                    }
                }
                State::ExpectValue => {
                    if byte.is_ascii_whitespace() {
                        i += 1;
                        continue;
                    }
                    self.value_start = at;
                    self.value_first = byte;
                    self.depth = 0;
                    self.in_string = false;
                    self.escaped = false;
                    self.state = State::InValue;
                    // A scalar ends at its own delimiter, so re-examine this
                    // byte as part of the value body.
                    self.step_value(byte, at);
                }
                State::InValue => self.step_value(byte, at),
                State::Done => break,
            }
            i += 1;
        }
        ControlFlow::Continue(())
    }

    /// Advance through a value's body, closing it when it ends.
    fn step_value(&mut self, byte: u8, at: u64) {
        if self.in_string {
            if self.escaped {
                self.escaped = false;
            } else if byte == b'\\' {
                self.escaped = true;
            } else if byte == b'"' {
                self.in_string = false;
                if self.depth == 0 {
                    // A bare string value ends at its closing quote.
                    self.close_value(at + 1);
                }
            }
            return;
        }

        match byte {
            b'"' => self.in_string = true,
            b'{' | b'[' => self.depth += 1,
            b'}' | b']' => {
                self.depth -= 1;
                if self.depth == 0 {
                    self.close_value(at + 1);
                } else if self.depth < 0 {
                    // The root object's closing brace. A bare scalar has no
                    // delimiter of its own, so this is where it ends -- without
                    // this the document's last key is silently dropped.
                    self.close_value(at);
                    self.state = State::Done;
                }
            }
            // A bare scalar (number, true, false, null) ends at the delimiter
            // that follows it — which also closes the root if it is `}`.
            b',' if self.depth == 0 => self.close_value(at),
            _ => {}
        }
    }

    fn close_value(&mut self, end: u64) {
        let kind = match self.value_first {
            b'[' => ValueKind::Array,
            b'{' => ValueKind::Object,
            _ => ValueKind::Scalar,
        };
        self.found.push(Collection {
            name: std::mem::take(&mut self.key),
            kind,
            start: self.value_start,
            end,
        });
        self.state = State::ExpectKey;
    }

    /// Close out a trailing scalar and yield what was found.
    fn finish(mut self) -> Option<JsonEnvelope> {
        if self.state == State::BeforeRoot {
            return None; // never saw a root object
        }
        // A truncated document can end inside a value; keep what was found.
        if self.state == State::InValue && self.depth == 0 {
            let end = self.at_eof;
            self.close_value(end);
        }
        Some(JsonEnvelope {
            collections: std::mem::take(&mut self.found),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn doc(contents: &str) -> NamedTempFile {
        let mut tmp = tempfile::Builder::new()
            .suffix(".json")
            .tempfile()
            .unwrap();
        tmp.write_all(contents.as_bytes()).unwrap();
        tmp.flush().unwrap();
        tmp
    }

    fn scan(contents: &str) -> JsonEnvelope {
        let file = doc(contents);
        JsonEnvelope::scan(file.path())
            .unwrap()
            .expect("an envelope")
    }

    /// The exact bytes a collection spans, to prove the ranges are usable.
    fn slice<'a>(contents: &'a str, c: &Collection) -> &'a str {
        &contents[c.start as usize..c.end as usize]
    }

    #[test]
    fn finds_top_level_keys_and_their_kinds() {
        let body = r#"{"meta":{"v":1},"users":[{"id":1}],"count":7,"name":"x"}"#;
        let env = scan(body);

        assert_eq!(
            env.collections
                .iter()
                .map(|c| (c.name.as_str(), c.kind))
                .collect::<Vec<_>>(),
            [
                ("meta", ValueKind::Object),
                ("users", ValueKind::Array),
                ("count", ValueKind::Scalar),
                ("name", ValueKind::Scalar),
            ]
        );
    }

    #[test]
    fn ranges_bound_exactly_the_value() {
        let body = r#"{"a":[1,2,3],"b":{"k":"v"},"c":42}"#;
        let env = scan(body);

        assert_eq!(slice(body, env.get("a").unwrap()), "[1,2,3]");
        assert_eq!(slice(body, env.get("b").unwrap()), r#"{"k":"v"}"#);
        assert_eq!(slice(body, env.get("c").unwrap()), "42");
    }

    #[test]
    fn nesting_does_not_end_a_value_early() {
        let body = r#"{"deep":[[{"a":[1,[2]]}],{"b":{"c":[]}}],"after":1}"#;
        let env = scan(body);

        assert_eq!(
            slice(body, env.get("deep").unwrap()),
            r#"[[{"a":[1,[2]]}],{"b":{"c":[]}}]"#
        );
        assert_eq!(slice(body, env.get("after").unwrap()), "1");
    }

    #[test]
    fn braces_inside_strings_are_not_structure() {
        // The classic way a byte scanner goes wrong.
        let body = r#"{"tricky":["}{][",":,"],"after":2}"#;
        let env = scan(body);

        assert_eq!(slice(body, env.get("tricky").unwrap()), r#"["}{][",":,"]"#);
        assert_eq!(slice(body, env.get("after").unwrap()), "2");
    }

    #[test]
    fn escaped_quotes_do_not_end_a_string() {
        let body = r#"{"s":"a \" }] b","after":3}"#;
        let env = scan(body);

        assert_eq!(slice(body, env.get("s").unwrap()), r#""a \" }] b""#);
        assert_eq!(slice(body, env.get("after").unwrap()), "3");
    }

    #[test]
    fn an_escaped_backslash_does_not_escape_the_next_quote() {
        let body = r#"{"s":"ends with a backslash \\","after":4}"#;
        let env = scan(body);

        assert_eq!(env.get("after").unwrap().kind, ValueKind::Scalar);
        assert_eq!(slice(body, env.get("after").unwrap()), "4");
    }

    #[test]
    fn whitespace_and_formatting_are_ignored() {
        let body = "{\n  \"users\" : [\n    {\"id\": 1}\n  ],\n  \"n\" : 2\n}\n";
        let env = scan(body);

        assert_eq!(env.collections.len(), 2);
        assert_eq!(env.get("users").unwrap().kind, ValueKind::Array);
        assert!(slice(body, env.get("users").unwrap()).starts_with('['));
        assert!(slice(body, env.get("users").unwrap()).ends_with(']'));
    }

    #[test]
    fn only_non_empty_arrays_are_queryable() {
        let env = scan(r#"{"rows":[{"a":1}],"empty":[],"obj":{"a":1},"n":1}"#);
        assert_eq!(
            env.queryable().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["rows"]
        );
    }

    #[test]
    fn a_document_that_is_not_an_object_has_no_envelope() {
        // Arrays and scalars are read natively; there is nothing to unwrap.
        let file = doc("[{\"a\":1}]");
        assert!(JsonEnvelope::scan(file.path()).unwrap().is_none());

        let file = doc("42");
        assert!(JsonEnvelope::scan(file.path()).unwrap().is_none());
    }

    #[test]
    fn an_empty_object_yields_no_collections() {
        assert!(scan("{}").collections.is_empty());
    }

    #[test]
    fn a_value_spanning_scan_chunks_is_still_bounded_correctly() {
        // The state machine carries across reads, so a collection larger than
        // the scan buffer must not be truncated.
        let filler: String = (0..SCAN_CHUNK / 8)
            .map(|i| format!("{{\"id\":{i}}},"))
            .collect();
        let body = format!("{{\"big\":[{}{{\"id\":-1}}],\"after\":9}}", filler);
        let file = doc(&body);
        let env = JsonEnvelope::scan(file.path()).unwrap().unwrap();

        let big = env.get("big").unwrap();
        assert_eq!(big.kind, ValueKind::Array);
        assert!(big.len() as usize > SCAN_CHUNK, "spans several chunks");
        assert_eq!(&body[big.start as usize..big.end as usize][..1], "[");
        assert_eq!(
            &body[(big.end - 1) as usize..big.end as usize],
            "]",
            "ends at its own closing bracket"
        );
        assert_eq!(env.get("after").unwrap().kind, ValueKind::Scalar);
    }

    #[test]
    fn a_string_spanning_scan_chunks_keeps_its_escape_state() {
        let long = "x".repeat(SCAN_CHUNK + 128);
        let body = format!("{{\"s\":\"{long}\",\"after\":5}}");
        let file = doc(&body);
        let env = JsonEnvelope::scan(file.path()).unwrap().unwrap();

        assert_eq!(env.collections.len(), 2);
        assert_eq!(env.get("after").unwrap().kind, ValueKind::Scalar);
    }

    #[test]
    #[ignore = "requires ~/Downloads/data_2gb.json"]
    fn scans_the_2gb_envelope() {
        let path = std::path::Path::new(concat!(env!("HOME"), "/Downloads/data_2gb.json"));
        if !path.exists() {
            return;
        }
        let started = std::time::Instant::now();
        let env = JsonEnvelope::scan(path).unwrap().expect("an envelope");
        println!("scan time: {:?}", started.elapsed());
        for c in &env.collections {
            println!(
                "  {:<12} {:?}  {:>6} MB",
                c.name,
                c.kind,
                c.len() / 1024 / 1024
            );
        }
        assert!(env.queryable().count() >= 3);
    }
}


#[cfg(test)]
mod phase_timing {
    use super::*;

    /// Split the cost of opening an envelope document into its two phases, so
    /// caching effort goes where the time actually is.
    #[test]
    #[ignore = "requires ~/Downloads/data_500mb.json"]
    fn scan_versus_stage() {
        let path = std::path::Path::new(concat!(env!("HOME"), "/Downloads/data_500mb.json"));
        if !path.exists() {
            return;
        }
        let t = std::time::Instant::now();
        let env = JsonEnvelope::scan(path).unwrap().expect("envelope");
        println!("SCAN:  {:?}", t.elapsed());

        let engine = crate::file::loaders::DuckdbConnection::new().unwrap();
        let t = std::time::Instant::now();
        for c in env.queryable() {
            engine.stage_collection(path, c).unwrap();
        }
        println!("STAGE: {:?} for {} collections", t.elapsed(), env.queryable().count());

        use crate::file::loaders::FileLoader as _;
        let t = std::time::Instant::now();
        let n = engine.len().unwrap();
        println!("COUNT: {:?} ({n} rows in the primary)", t.elapsed());
    }
}
