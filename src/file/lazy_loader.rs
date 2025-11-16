use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

#[cfg(unix)]
use std::os::unix::fs::FileExt; // read_at
#[cfg(windows)]
use std::os::windows::fs::FileExt; // seek_read

use crate::file::detect_file_type::{DetectedFileType, sniff_file_type};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FileType {
    #[default]
    Ndjson,
    Json, // expects top-level array [ ... ] or single object
}

impl From<DetectedFileType> for FileType {
    fn from(val: DetectedFileType) -> Self {
        match val {
            DetectedFileType::Ndjson => FileType::Ndjson,
            DetectedFileType::JsonArray | DetectedFileType::JsonObject => FileType::Json,
        }
    }
}

impl From<LazyJsonFile> for FileType {
    fn from(val: LazyJsonFile) -> Self {
        match val {
            LazyJsonFile::Ndjson(_) => FileType::Ndjson,
            LazyJsonFile::JsonArray(_) | LazyJsonFile::Single(_) => FileType::Json,
        }
    }
}

pub enum LazyJsonFile {
    Ndjson(NdjsonFile),
    JsonArray(JsonArrayFile),
    Single(SingleValueFile),
}

impl LazyJsonFile {
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match self {
            LazyJsonFile::Ndjson(f) => f.len(),
            LazyJsonFile::JsonArray(f) => f.len(),
            LazyJsonFile::Single(_) => 1,
        }
    }

    pub fn get(&mut self, idx: usize) -> Result<Value> {
        match self {
            LazyJsonFile::Ndjson(f) => f.get(idx),
            LazyJsonFile::JsonArray(f) => f.get(idx),
            LazyJsonFile::Single(f) => f.get(idx),
        }
    }

    pub fn raw_slice(&self, idx: usize) -> Result<Vec<u8>> {
        match self {
            LazyJsonFile::Ndjson(f) => f.raw_line(idx),
            LazyJsonFile::JsonArray(f) => f.raw_element(idx),
            LazyJsonFile::Single(f) => f.raw_all(),
        }
    }
}

/* ---------------- NDJSON ---------------- */

pub struct NdjsonFile {
    file: File,
    // (start, end) byte offsets for each line (end is exclusive)
    line_spans: Vec<(u64, u64)>,
}

impl NdjsonFile {
    /// Position-independent line read (safe for parallel).
    pub fn raw_line(&self, idx: usize) -> Result<Vec<u8>> {
        let (start, end) = *self.line_spans.get(idx).ok_or_else(|| anyhow!("oob"))?;
        let len = (end - start) as usize;
        let mut buf = vec![0u8; len];

        #[cfg(unix)]
        {
            self.file.read_at(&mut buf, start)?;
        }
        #[cfg(windows)]
        {
            self.file.seek_read(&mut buf, start)?;
        }

        // Trim trailing CRLF/LF if your spans included it; with this indexer we exclude '\n' already.
        Ok(buf)
    }

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

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.line_spans.len()
    }

    pub fn get(&mut self, idx: usize) -> Result<Value> {
        // Read the exact span and parse (no shared cursor)
        let (start, end) = *self
            .line_spans
            .get(idx)
            .ok_or_else(|| anyhow!("index out of bounds"))?;
        let len = (end - start) as usize;
        let mut buf = vec![0u8; len];

        #[cfg(unix)]
        {
            self.file.read_at(&mut buf, start)?;
        }
        #[cfg(windows)]
        {
            self.file.seek_read(&mut buf, start)?;
        }

        let v: Value = serde_json::from_slice(&buf)
            .with_context(|| format!("invalid JSON at line index {}", idx))?;
        Ok(v)
    }
}

/* ---------------- JSON array: [ {...}, {...}, ... ] ---------------- */

pub struct JsonArrayFile {
    file: File,
    element_spans: Vec<(u64, u64)>, // (start, end) exclusive
}

impl JsonArrayFile {
    /// Position-independent element read (safe for parallel).
    pub fn raw_element(&self, idx: usize) -> Result<Vec<u8>> {
        let (start, end) = *self.element_spans.get(idx).ok_or_else(|| anyhow!("oob"))?;
        let len = (end - start) as usize;
        let mut buf = vec![0u8; len];

        #[cfg(unix)]
        {
            self.file.read_at(&mut buf, start)?;
        }
        #[cfg(windows)]
        {
            self.file.seek_read(&mut buf, start)?;
        }

        Ok(buf)
    }

    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path).with_context(|| "open JSON")?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        // Find the top-level array and index each element span without parsing it.
        let spans = index_json_array_elements(&buf)
            .map_err(|e| anyhow!("failed to index top-level array: {e}"))?;

        // Keep the file for later slice reads
        let file = File::open(path)?;
        Ok(Self {
            file,
            element_spans: spans,
        })
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.element_spans.len()
    }

    pub fn get(&mut self, idx: usize) -> Result<Value> {
        let (start, end) = *self
            .element_spans
            .get(idx)
            .ok_or_else(|| anyhow!("index out of bounds"))?;
        let len = (end - start) as usize;
        let mut buf = vec![0u8; len];

        #[cfg(unix)]
        {
            self.file.read_at(&mut buf, start)?;
        }
        #[cfg(windows)]
        {
            self.file.seek_read(&mut buf, start)?;
        }

        let v: Value = serde_json::from_slice(&buf)
            .with_context(|| format!("invalid element at index {}", idx))?;
        Ok(v)
    }
}

/* ---------------- Single value (whole file) ---------------- */

pub struct SingleValueFile {
    file: File,
    parsed: Option<Value>,
}

impl SingleValueFile {
    /// Position-independent whole-file read (safe for parallel).
    pub fn raw_all(&self) -> Result<Vec<u8>> {
        let len = self.file.metadata()?.len() as usize;
        let mut buf = vec![0u8; len];

        #[cfg(unix)]
        {
            self.file.read_at(&mut buf, 0)?;
        }
        #[cfg(windows)]
        {
            self.file.seek_read(&mut buf, 0)?;
        }

        Ok(buf)
    }

    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            file: File::open(path)?,
            parsed: None,
        })
    }

    pub fn get(&mut self, idx: usize) -> Result<Value> {
        if idx != 0 {
            anyhow::bail!("index out of bounds");
        }
        if let Some(v) = self.parsed.as_ref() {
            return Ok(v.clone());
        }

        // Read full file via position-independent I/O, then parse.
        let len = self.file.metadata()?.len() as usize;
        let mut buf = vec![0u8; len];

        #[cfg(unix)]
        {
            self.file.read_at(&mut buf, 0)?;
        }
        #[cfg(windows)]
        {
            self.file.seek_read(&mut buf, 0)?;
        }

        let v: Value = serde_json::from_slice(&buf)?;
        self.parsed = Some(v.clone());
        Ok(v)
    }
}

/* ---------------- Array element indexer ---------------- */

fn index_json_array_elements(bytes: &[u8]) -> Result<Vec<(u64, u64)>> {
    // Skip leading whitespace
    let mut i = skip_ws(bytes, 0).ok_or_else(|| anyhow!("empty file"))?;
    if bytes.get(i) != Some(&b'[') {
        return Err(anyhow!("expected a top-level JSON array"));
    }
    i += 1;

    let mut spans = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut depth: i32 = 0; // depth relative to inside the array
    let mut elem_start: Option<u64> = None;
    let mut want_new_elem = true;

    while i < bytes.len() {
        let b = bytes[i];

        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        match b {
            b'"' => {
                if want_new_elem {
                    elem_start = Some(i as u64);
                    want_new_elem = false;
                }
                in_string = true;
            }
            b'{' | b'[' => {
                if want_new_elem {
                    elem_start = Some(i as u64);
                    want_new_elem = false;
                }
                depth += 1;
            }
            b'}' | b']' => {
                if depth == 0 {
                    if b == b']' {
                        if let Some(s) = elem_start {
                            let end = prev_non_ws(bytes, i).unwrap_or(i) + 1;
                            spans.push((s, end as u64));
                        }
                        break;
                    }
                } else {
                    depth -= 1;
                }
            }
            b',' => {
                if depth == 0 {
                    if let Some(s) = elem_start.take() {
                        let end = prev_non_ws(bytes, i).unwrap_or(i);
                        spans.push((s, (end + 1) as u64));
                    }
                    want_new_elem = true;
                }
            }
            c if is_ws(c) => { /* skip */ }
            _ => {
                if want_new_elem {
                    elem_start = Some(i as u64);
                    want_new_elem = false;
                }
            }
        }

        i += 1;
    }

    // Minimal sanity check: we should have encountered a ']'
    if !bytes.contains(&b']') {
        return Err(anyhow!("unterminated top-level array"));
    }

    Ok(spans)
}

#[inline]
fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\n' | b'\r' | b'\t')
}

fn skip_ws(bytes: &[u8], mut i: usize) -> Option<usize> {
    while i < bytes.len() && is_ws(bytes[i]) {
        i += 1;
    }
    (i < bytes.len()).then_some(i)
}

fn prev_non_ws(bytes: &[u8], mut i: usize) -> Option<usize> {
    if i == 0 {
        return None;
    }
    i -= 1;
    while i > 0 && is_ws(bytes[i]) {
        i -= 1;
    }
    Some(i)
}

/* ---------------- Public entry ---------------- */

pub fn load_file_auto(path: &Path) -> Result<(DetectedFileType, LazyJsonFile)> {
    let detected = sniff_file_type(path)?;
    let lazy = match detected {
        DetectedFileType::Ndjson => LazyJsonFile::Ndjson(NdjsonFile::open(path)?),
        DetectedFileType::JsonArray => LazyJsonFile::JsonArray(JsonArrayFile::open(path)?),
        DetectedFileType::JsonObject => LazyJsonFile::Single(SingleValueFile::open(path)?),
    };
    Ok((detected, lazy))
}
