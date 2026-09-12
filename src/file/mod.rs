use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub mod detect_file_type;
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
            Some("db") | Some("duckdb") | Some("sqlite") | Some("sqlite3") => FileType::DB,
            Some("duckdb_extension") | Some("so") | Some("dll") | Some("dylib") => FileType::Plugin,
            _ => FileType::Unknown,
        }
    }

    /// Short label for UI / logs.
    pub fn label(&self) -> &'static str {
        match self {
            FileType::Json => "JSON",
            FileType::Csv => "CSV",
            FileType::Parquet => "Parquet",
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
