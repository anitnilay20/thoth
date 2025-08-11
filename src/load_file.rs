use anyhow::Result;
use serde_json::Value;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use crate::FileType;

pub fn load_file(path: &PathBuf, file_type: &FileType) -> Result<Vec<serde_json::Value>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    match file_type {
        FileType::Ndjson => {
            let mut lines = vec![];
            for line in reader.lines() {
                let line = line?;
                let json: Value = serde_json::from_str(&line)?;
                // lines.push(Value::String(line));
                lines.push(json);
            }
            Ok(lines)
        }
        FileType::Json => {
            let json: Value = serde_json::from_reader(reader)?;
            Ok(
                vec![json], // Wrap in a vector for consistency with NDJSON
            )
        }
    }
}
