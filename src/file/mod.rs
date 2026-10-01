use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub mod detect_file_type;
pub mod extensions;
pub mod index_cache;
pub mod indexing;
pub mod json_envelope;
pub mod loaders;
pub mod to_dataset;

pub use loaders::FileKind;
pub use loaders::duck_db::DuckdbConnection;

/// How a file is read — decided by magic bytes, then extension.
///
/// This is *detection only*. The loader that acts on it is
/// [`DuckdbConnection`], which maps each variant to the right DuckDB reader
/// (or to the plugin staging path for [`FileType::Plugin`]).
#[derive(Debug, PartialEq, Eq, Copy, Clone, Default)]
pub enum FileType {
    Json,
    Csv,
    Parquet,
    Excel,
    Arrow,
    Avro,
    DB,
    Plugin,
    #[default]
    Unknown,
}

impl FileType {
    /// Detect a file's type from its path: magic bytes first, then extension.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();

        // 1. Magic bytes — the reliable signals (DB and Parquet).
        if let Ok(mut file) = File::open(path) {
            let mut head = [0u8; 16];
            if let Ok(n) = file.read(&mut head) {
                // SQLite database.
                if n >= 16 && &head == b"SQLite format 3\0" {
                    return FileType::DB;
                }
                // DuckDB database: "DUCK" at offset 8.
                if n >= 12 && &head[8..12] == b"DUCK" {
                    return FileType::DB;
                }
                // Parquet: leading "PAR1" (confirm trailing magic too).
                if n >= 4 && &head[0..4] == b"PAR1" && has_trailing_par1(&mut file) {
                    return FileType::Parquet;
                }
                // Avro object container file: "Obj" and the format's version
                // byte. The schema follows in the header, which is why an
                // `.avro` needs no sniffing beyond this.
                if n >= 4 && &head[0..4] == b"Obj\x01" {
                    return FileType::Avro;
                }
            }
        }

        // 2. Extension fallback for everything else.
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("json") | Some("ndjson") | Some("jsonl") => FileType::Json,
            Some("csv") | Some("tsv") => FileType::Csv,
            Some("parquet") | Some("pq") => FileType::Parquet,
            Some("xlsx") | Some("xlsm") => FileType::Excel,
            Some("arrow") | Some("arrows") | Some("ipc") => FileType::Arrow,
            Some("avro") => FileType::Avro,
            Some("db") | Some("duckdb") | Some("sqlite") | Some("sqlite3") => FileType::DB,
            Some("duckdb_extension") | Some("so") | Some("dll") | Some("dylib") => FileType::Plugin,
            _ => FileType::Unknown,
        }
    }

    /// Whether the engine reads this format itself.
    ///
    /// The engine is the better reader for everything it claims — lazy
    /// scanning, real types, and SQL over the result — so a plugin never gets
    /// a format from this list, even one that asks for it. Without that rule
    /// a bundled loader silently shadows DuckDB: the CSV plugin declared
    /// `file-viewer` for `.csv`, and every spreadsheet in the app went through
    /// it instead, arriving with no query builder, no collections and no
    /// chart.
    ///
    /// [`Unknown`](FileType::Unknown) is absent deliberately: the extension
    /// has said nothing, so a plugin is asked only after the engine's own
    /// readers have declined (see `DuckdbConnection::sniff_reader`).
    pub fn is_native(&self) -> bool {
        matches!(
            self,
            FileType::Json
                | FileType::Csv
                | FileType::Parquet
                | FileType::Excel
                | FileType::Arrow
                | FileType::Avro
                | FileType::DB
        )
    }

    /// Whether this path is a prose document rather than data.
    ///
    /// Markdown is the case that matters: DuckDB's CSV sniffer will read
    /// almost any line-oriented text as a one-column table, and a README that
    /// opens as a grid of its own lines is worse than no table at all. A
    /// document is read as a document.
    pub fn is_prose_document<P: AsRef<Path>>(path: P) -> bool {
        matches!(
            path.as_ref()
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("md" | "markdown" | "mdown" | "mkd")
        )
    }

    /// Short label for UI / logs.
    pub fn label(&self) -> &'static str {
        match self {
            FileType::Json => "JSON",
            FileType::Csv => "CSV",
            FileType::Parquet => "Parquet",
            FileType::Excel => "Excel",
            FileType::Arrow => "Arrow",
            FileType::Avro => "Avro",
            FileType::DB => "Database",
            FileType::Plugin => "Plugin",
            FileType::Unknown => "Unknown",
        }
    }
}

impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Confirm a Parquet file ends with the "PAR1" trailer.
fn has_trailing_par1(file: &mut File) -> bool {
    let Ok(len) = file.seek(SeekFrom::End(0)) else {
        return false;
    };
    if len < 8 {
        return false;
    }
    if file.seek(SeekFrom::End(-4)).is_err() {
        return false;
    }
    let mut tail = [0u8; 4];
    file.read_exact(&mut tail).is_ok() && &tail == b"PAR1"
}

/// A file in the user's Downloads that a measurement test wants, or `None`
/// when it is not there.
///
/// Resolved at *run* time, and through `dirs` rather than `HOME`. These tests
/// are `#[ignore]`d and skip themselves when the file is absent, but
/// `env!("HOME")` is read when the crate is compiled and Windows has no such
/// variable — so six of them failed the whole test build on a platform where
/// they never run.
#[cfg(test)]
pub(crate) fn downloaded(name: &str) -> Option<std::path::PathBuf> {
    let path = dirs::home_dir()?.join("Downloads").join(name);
    path.exists().then_some(path)
}

/// Build a minimal Avro object container file.
///
/// Written out by hand rather than pulled from a crate or a committed blob:
/// the format is a header (`Obj\x01`, a metadata map carrying the writer's
/// schema, a 16-byte sync marker) followed by blocks of records, and a test
/// that assembles one checks the detector against the bytes the spec calls
/// for rather than against a fixture nobody in the repo can read.
#[cfg(test)]
pub(crate) fn avro_container(rows: &[(&str, i64)]) -> Vec<u8> {
    const SCHEMA: &str = concat!(
        r#"{"type":"record","name":"r","fields":["#,
        r#"{"name":"name","type":"string"},{"name":"id","type":"long"}]}"#
    );

    /// Avro writes integers zigzag-encoded, then as a LEB128 varint.
    fn long(n: i64, out: &mut Vec<u8>) {
        let mut v = ((n << 1) ^ (n >> 63)) as u64;
        loop {
            if v & !0x7f == 0 {
                out.push(v as u8);
                return;
            }
            out.push(((v & 0x7f) | 0x80) as u8);
            v >>= 7;
        }
    }

    /// A string and a byte string share one encoding: a length, then the bytes.
    fn bytes(s: &str, out: &mut Vec<u8>) {
        long(s.len() as i64, out);
        out.extend_from_slice(s.as_bytes());
    }

    // Any 16 bytes will do, so long as the header and every block agree.
    let sync = [0x42u8; 16];

    let mut out = b"Obj\x01".to_vec();
    long(2, &mut out); // two metadata entries ...
    bytes("avro.schema", &mut out);
    bytes(SCHEMA, &mut out);
    bytes("avro.codec", &mut out);
    bytes("null", &mut out);
    long(0, &mut out); // ... and the empty block that ends the map
    out.extend_from_slice(&sync);

    let mut block = Vec::new();
    for (name, id) in rows {
        bytes(name, &mut block);
        long(*id, &mut block);
    }
    long(rows.len() as i64, &mut out);
    long(block.len() as i64, &mut out);
    out.extend_from_slice(&block);
    out.extend_from_slice(&sync);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_avro_container_is_known_by_its_bytes_not_its_name() {
        // Magic bytes come first for a reason: a file named `.bin` is still
        // the format its header says it is.
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("records.bin");
        std::fs::write(&path, avro_container(&[("alpha", 1), ("beta", -2)]))
            .expect("writing the container");
        assert_eq!(FileType::from_path(&path), FileType::Avro);
    }

    #[test]
    fn an_avro_file_is_known_by_its_extension_too() {
        // Nothing to sniff — an empty file still has a name, and the name is
        // what the user chose when they saved it.
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("records.avro");
        std::fs::write(&path, b"").expect("writing an empty file");
        assert_eq!(FileType::from_path(&path), FileType::Avro);
    }

    #[test]
    fn a_header_that_only_starts_like_avro_is_not_avro() {
        // "Obj" without the version byte is some other file beginning with a
        // word. The fourth byte is what makes it a container.
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("notes.bin");
        std::fs::write(&path, b"Object storage notes").expect("writing the file");
        assert_ne!(FileType::from_path(&path), FileType::Avro);
    }

    #[test]
    fn the_engine_claims_avro_as_its_own() {
        // `is_native` is what stops a plugin shadowing DuckDB's reader.
        assert!(FileType::Avro.is_native());
        assert_eq!(FileType::Avro.label(), "Avro");
    }
}
