use crate::error::{Result, ThothError};
use crate::file::loaders::FileLoader;
use crate::platform::FileIO;
use serde_json::Value;
use std::{fs::File, path::Path};

/// Lazy loader for JSON files containing a single top-level value
///
/// This loader handles files containing a single JSON object or value.
/// The value is parsed on first access and cached for subsequent accesses.
pub struct SingleValueFile {
    file: File,
    parsed: Option<Value>,
}

impl SingleValueFile {
    /// Open a single-value JSON file
    ///
    /// The file is not parsed immediately; parsing happens on the first
    /// call to `get()`.
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            file: File::open(path)?,
            parsed: None,
        })
    }

    /// Get the parsed JSON value (always at index 0)
    ///
    /// This performs a position-independent read and is safe for parallel access.
    /// The parsed value is cached after the first access.
    pub fn get(&mut self, idx: usize) -> Result<Value> {
        if idx != 0 {
            return Err(ThothError::InvalidJsonStructure {
                reason: format!("Single JSON object only has index 0, got {}", idx),
            });
        }
        if let Some(v) = self.parsed.as_ref() {
            return Ok(v.clone());
        }

        // Read full file via position-independent I/O, then parse.
        let len = self.file.metadata()?.len() as usize;
        let mut buf = vec![0u8; len];
        self.file.read_at(&mut buf, 0)?;

        let v: Value = serde_json::from_slice(&buf)?;
        self.parsed = Some(v.clone());
        Ok(v)
    }

    /// Get raw bytes for the entire file
    ///
    /// This performs a position-independent read and is safe for parallel access.
    pub fn raw_all(&self) -> Result<Vec<u8>> {
        let len = self.file.metadata()?.len() as usize;
        let mut buf = vec![0u8; len];
        self.file.read_at(&mut buf, 0)?;

        Ok(buf)
    }
}

impl FileLoader for SingleValueFile {
    type Item = Value;

    fn open(path: &Path) -> Result<Self> {
        SingleValueFile::open(path)
    }

    fn len(&self) -> usize {
        1 // Single value files always have exactly one element
    }

    fn get(&mut self, idx: usize) -> Result<Self::Item> {
        self.get(idx)
    }

    fn raw_bytes(&self, idx: usize) -> Result<Vec<u8>> {
        if idx != 0 {
            return Err(ThothError::InvalidJsonStructure {
                reason: format!("Single JSON object only has index 0, got {}", idx),
            });
        }
        self.raw_all()
    }
}
