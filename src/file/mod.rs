use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::Result;
use crate::file::loaders::{FileLoader, duck_db::DuckdbConnection};

// pub mod lazy_loader;
pub mod loaders;
pub mod to_dataset;

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum FileType {
    Json,
    Csv,
    Parquet,
    DB,
    Plugin,
    Unknown,
}

impl Default for FileType {
    fn default() -> Self {
        FileType::Unknown
    }
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

impl FileLoader for FileType {
    fn query(&self, query: &str) -> Result<Vec<duckdb::arrow::array::RecordBatch>> {
        match self {
            FileType::Json | FileType::Csv | FileType::Parquet | FileType::DB => {
                let db = DuckdbConnection::new()?;
                db.query(query)
            }
            FileType::Plugin => todo!(),
            FileType::Unknown => todo!(),
        }
    }

    fn fetch(
        &self,
        filters: Vec<String>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<duckdb::arrow::array::RecordBatch>> {
        match self {
            FileType::Json | FileType::Csv | FileType::Parquet | FileType::DB => {
                let db = DuckdbConnection::new()?;
                db.fetch(filters, offset, limit)
            }
            FileType::Plugin => todo!(),
            FileType::Unknown => todo!(),
        }
    }

    fn size(&self) -> Result<u128> {
        match self {
            FileType::Json | FileType::Csv | FileType::Parquet | FileType::DB => {
                let db = DuckdbConnection::new()?;
                db.size()
            }
            FileType::Plugin => todo!(),
            FileType::Unknown => todo!(),
        }
    }

    fn len(&self) -> Result<usize> {
        match self {
            FileType::Json | FileType::Csv | FileType::Parquet | FileType::DB => {
                let db = DuckdbConnection::new()?;
                db.len()
            }
            FileType::Plugin => todo!(),
            FileType::Unknown => todo!(),
        }
    }

    fn get(&self, index: usize) -> Result<duckdb::arrow::array::RecordBatch> {
        match self {
            FileType::Json | FileType::Csv | FileType::Parquet | FileType::DB => {
                let db = DuckdbConnection::new()?;
                db.get(index)
            }
            FileType::Plugin => todo!(),
            FileType::Unknown => todo!(),
        }
    }

    fn open(&self, path: &str, alias: &str) -> Result<()> {
        match self {
            FileType::Json | FileType::Csv | FileType::Parquet | FileType::DB => {
                let db = DuckdbConnection::new()?;
                db.open(path, alias)
            }
            FileType::Plugin => todo!(),
            FileType::Unknown => todo!(),
        }
    }
}
