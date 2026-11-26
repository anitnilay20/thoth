//! Format conversion and batch export over DuckDB (#10).
//!
//! Converts any format readable by [`DuckdbConnection`] (JSON, NDJSON, CSV,
//! Parquet, Excel, Arrow, Avro, SQLite) into `json`, `ndjson`, `csv`, or
//! `parquet` — optionally filtered/shaped by a SQL `SELECT` query.

use std::path::{Path, PathBuf};

use crate::error::{Result, ThothError};
use crate::file::loaders::FileLoader;
use crate::file::loaders::duck_db::DuckdbConnection;

/// Supported output formats for [`convert_file`] and [`convert_batch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Ndjson,
    Csv,
    Parquet,
}

impl ExportFormat {
    pub fn from_name(name: &str) -> Result<Self> {
        match name.trim().trim_start_matches('.').to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "ndjson" | "jsonl" => Ok(Self::Ndjson),
            "csv" => Ok(Self::Csv),
            "parquet" | "pq" => Ok(Self::Parquet),
            other => Err(ThothError::Unknown {
                message: format!(
                    "unsupported export format '{other}'; expected json, ndjson, csv, or parquet"
                ),
            }),
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Ndjson => "ndjson",
            Self::Csv => "csv",
            Self::Parquet => "parquet",
        }
    }
}

fn quote_sql_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    format!("'{}'", s.replace('\'', "''"))
}

/// Convert `input` to `output` in `format`, optionally running `sql` (which
/// can reference the primary table alias `data`) instead of `SELECT * FROM data`.
pub fn convert_file(
    input: &Path,
    output: &Path,
    format: ExportFormat,
    sql: Option<&str>,
) -> Result<()> {
    let db = DuckdbConnection::new()?;
    db.open(&input.to_string_lossy(), "data")?;

    let select_sql = sql
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("SELECT * FROM data");
    let out_lit = quote_sql_path(output);

    let copy_stmt = match format {
        ExportFormat::Json => format!("COPY ({select_sql}) TO {out_lit} (FORMAT JSON, ARRAY TRUE)"),
        ExportFormat::Ndjson => format!("COPY ({select_sql}) TO {out_lit} (FORMAT JSON)"),
        ExportFormat::Csv => format!("COPY ({select_sql}) TO {out_lit} (FORMAT CSV, HEADER TRUE)"),
        ExportFormat::Parquet => format!("COPY ({select_sql}) TO {out_lit} (FORMAT PARQUET)"),
    };

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    db.query(&copy_stmt)?;
    Ok(())
}

/// Batch-convert multiple `inputs` into `out_dir` using `format`, returning
/// the list of written output paths.
pub fn convert_batch(
    inputs: &[PathBuf],
    out_dir: &Path,
    format: ExportFormat,
    sql: Option<&str>,
) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(out_dir)?;
    let mut outputs = Vec::with_capacity(inputs.len());
    for input in inputs {
        let stem = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let out_path = out_dir.join(format!("{stem}.{}", format.extension()));
        convert_file(input, &out_path, format, sql)?;
        outputs.push(out_path);
    }
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::Builder;

    #[test]
    fn converts_csv_to_json_ndjson_and_parquet() {
        let mut src = Builder::new().suffix(".csv").tempfile().unwrap();
        writeln!(src, "name,age\nada,36\nlinus,54").unwrap();
        src.flush().unwrap();

        let dir = tempfile::tempdir().unwrap();
        let json_out = dir.path().join("out.json");
        convert_file(src.path(), &json_out, ExportFormat::Json, None).unwrap();
        let json_text = std::fs::read_to_string(&json_out).unwrap();
        assert!(json_text.contains("ada") && json_text.contains("linus"));

        let ndjson_out = dir.path().join("out.ndjson");
        convert_file(
            src.path(),
            &ndjson_out,
            ExportFormat::Ndjson,
            Some("SELECT name FROM data WHERE age > 40"),
        )
        .unwrap();
        let ndjson_text = std::fs::read_to_string(&ndjson_out).unwrap();
        assert!(ndjson_text.contains("linus") && !ndjson_text.contains("ada"));

        let pq_out = dir.path().join("out.parquet");
        convert_file(src.path(), &pq_out, ExportFormat::Parquet, None).unwrap();
        assert!(std::fs::metadata(&pq_out).unwrap().len() > 0);
    }

    #[test]
    fn batch_converts_multiple_files() {
        let mut f1 = Builder::new().suffix(".csv").tempfile().unwrap();
        writeln!(f1, "a\n1").unwrap();
        let mut f2 = Builder::new().suffix(".csv").tempfile().unwrap();
        writeln!(f2, "b\n2").unwrap();

        let out_dir = tempfile::tempdir().unwrap();
        let outputs = convert_batch(
            &[f1.path().to_path_buf(), f2.path().to_path_buf()],
            out_dir.path(),
            ExportFormat::Json,
            None,
        )
        .unwrap();
        assert_eq!(outputs.len(), 2);
        assert!(outputs.iter().all(|p| p.exists()));
    }
}
