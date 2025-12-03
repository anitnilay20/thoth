use crate::error::{Result, ThothError};
use crate::file::loaders::FileLoader;
use crate::platform::FileIO;
use anyhow::Context;
use serde_json::Value;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

/// Lazy loader for NDJSON (Newline Delimited JSON) files
///
/// This loader indexes line boundaries during initialization, allowing
/// for efficient random access to individual JSON objects without loading
/// the entire file into memory.
pub struct NdjsonFile {
    file: File,
    // (start, end) byte offsets for each line (end is exclusive)
    line_spans: Vec<(u64, u64)>,
}

impl NdjsonFile {
    /// Open an NDJSON file and index all line boundaries
    ///
    /// This performs a single streaming pass to build an index of line spans,
    /// which allows for efficient random access later.
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| "open NDJSON")?;

        // Build (start,end) for each line using a single streaming pass
        let mut spans = Vec::new();
        let mut reader = BufReader::new(file.try_clone()?);
        let mut pos: u64 = 0;
        let mut buf = Vec::with_capacity(8 * 1024);
        loop {
            buf.clear();
            let n = reader.read_until(b'\n', &mut buf)?;
            if n == 0 {
                break;
            }

            // Exclude the '\n' from the span (common for substring search)
            let end_exclusive = if buf.last() == Some(&b'\n') {
                pos + (n as u64) - 1
            } else {
                pos + (n as u64)
            };

            // Also strip trailing '\r' if present (CRLF files)
            let (start, mut end) = (pos, end_exclusive);
            if end > start {
                // Read last byte of this slice to check for '\r'
                // (We don't have the bytes here, but we can detect from buf)
                if buf.len() >= 2 && buf[buf.len() - 2] == b'\r' && buf[buf.len() - 1] == b'\n' {
                    end -= 1;
                }
            }

            spans.push((start, end));
            pos += n as u64;
        }

        Ok(Self {
            file,
            line_spans: spans,
        })
    }

    /// Returns the number of lines (JSON objects) in the file
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.line_spans.len()
    }

    /// Get a parsed JSON value at the specified line index
    ///
    /// This performs a position-independent read and is safe for parallel access.
    pub fn get(&mut self, idx: usize) -> Result<Value> {
        // Read the exact span and parse (no shared cursor)
        let (start, end) =
            *self
                .line_spans
                .get(idx)
                .ok_or_else(|| ThothError::InvalidJsonStructure {
                    reason: format!("NDJSON line index {} out of bounds", idx),
                })?;
        let len = (end - start) as usize;
        let mut buf = vec![0u8; len];
        self.file.read_at(&mut buf, start)?;

        let v: Value = serde_json::from_slice(&buf)
            .with_context(|| format!("invalid JSON at line index {}", idx))?;
        Ok(v)
    }

    /// Get raw bytes for a line at the specified index
    ///
    /// This performs a position-independent read and is safe for parallel access.
    pub fn raw_line(&self, idx: usize) -> Result<Vec<u8>> {
        let (start, end) =
            *self
                .line_spans
                .get(idx)
                .ok_or_else(|| ThothError::InvalidJsonStructure {
                    reason: format!("Line index {} out of bounds", idx),
                })?;
        let len = (end - start) as usize;
        let mut buf = vec![0u8; len];
        self.file.read_at(&mut buf, start)?;

        Ok(buf)
    }
}

impl FileLoader for NdjsonFile {
    type Item = Value;

    fn open(path: &Path) -> Result<Self> {
        NdjsonFile::open(path)
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn get(&mut self, idx: usize) -> Result<Self::Item> {
        self.get(idx)
    }

    fn raw_bytes(&self, idx: usize) -> Result<Vec<u8>> {
        self.raw_line(idx)
    }
}
