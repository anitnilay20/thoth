use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use crate::file::detect_file_type::{DetectedFileType, sniff_file_type};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FileType {
    #[default]
    Ndjson,
    Json, // expects top-level array [ ... ]
}

impl From<DetectedFileType> for FileType {
    fn from(val: DetectedFileType) -> Self {
        match val {
            DetectedFileType::Ndjson => FileType::Ndjson,
            DetectedFileType::JsonArray => FileType::Json,
            DetectedFileType::JsonObject => FileType::Json,
        }
    }
}

pub enum LazyJsonFile {
    Ndjson(NdjsonFile),
    JsonArray(JsonArrayFile),
    Single(SingleValueFile),
}

impl LazyJsonFile {
    pub fn len(&self) -> usize {
        match self {
            LazyJsonFile::Ndjson(f) => f.len(),
            LazyJsonFile::JsonArray(f) => f.len(),
            LazyJsonFile::Single(_) => 1,
        }
    }

    /// Read and parse item at index into a serde_json::Value.
    pub fn get(&mut self, idx: usize) -> Result<Value> {
        match self {
            LazyJsonFile::Ndjson(f) => f.get(idx),
            LazyJsonFile::JsonArray(f) => f.get(idx),
            LazyJsonFile::Single(f) => f.get(idx),
        }
    }

    /// Convenience: fetch a slice [start, start+count)
    pub fn get_range(&mut self, start: usize, count: usize) -> Result<Vec<Value>> {
        let end = start.saturating_add(count).min(self.len());
        let mut out = Vec::with_capacity(end.saturating_sub(start));
        for i in start..end {
            out.push(self.get(i)?);
        }
        Ok(out)
    }
}

/* ---------------- NDJSON (best performance & simplest) ---------------- */

pub struct NdjsonFile {
    file: File,
    line_offsets: Vec<u64>,
}

impl NdjsonFile {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| "open NDJSON")?;
        let mut reader = BufReader::new(file.try_clone()?);

        let mut offsets = Vec::new();
        let mut pos: u64 = 0;
        let mut buf = Vec::with_capacity(8 * 1024);

        loop {
            buf.clear();
            let n = reader.read_until(b'\n', &mut buf)?;
            if n == 0 {
                break;
            }
            offsets.push(pos);
            pos += n as u64;
        }

        Ok(Self {
            file,
            line_offsets: offsets,
        })
    }

    pub fn len(&self) -> usize {
        self.line_offsets.len()
    }

    pub fn get(&mut self, idx: usize) -> Result<Value> {
        let start = *self
            .line_offsets
            .get(idx)
            .ok_or_else(|| anyhow!("index out of bounds"))?;
        // Seek to start and read the line
        self.file.seek(SeekFrom::Start(start))?;
        let mut reader = BufReader::new(self.file.try_clone()?);
        let mut line = String::new();
        let _ = reader.read_line(&mut line)?;
        // Trim trailing newline without copying large strings
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        let v: Value = serde_json::from_str(&line)
            .with_context(|| format!("invalid JSON at line index {}", idx))?;
        Ok(v)
    }
}

/* ---------------- JSON array: [ {...}, {...}, ... ] ----------------
   We index the start..end byte-ranges of each top-level element using
   a single pass with a tiny state machine (string/escape/bracket depth).
-------------------------------------------------------------------- */

pub struct JsonArrayFile {
    file: File,
    element_spans: Vec<(u64, u64)>, // (start, end) byte offsets (inclusive start, exclusive end)
}

impl JsonArrayFile {
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

    pub fn len(&self) -> usize {
        self.element_spans.len()
    }

    pub fn get(&mut self, idx: usize) -> Result<Value> {
        let (start, end) = *self
            .element_spans
            .get(idx)
            .ok_or_else(|| anyhow!("index out of bounds"))?;

        // Read just the element slice and parse it.
        let len = (end - start) as usize;
        let mut buf = vec![0u8; len];
        self.file.seek(SeekFrom::Start(start))?;
        std::io::Read::read_exact(&mut self.file, &mut buf)?;
        let v: Value = serde_json::from_slice(&buf)
            .with_context(|| format!("invalid element at index {}", idx))?;
        Ok(v)
    }
}

pub struct SingleValueFile {
    file: File,
    parsed: Option<Value>,
}

impl SingleValueFile {
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
            return Ok(v.clone()); // cheap in practice; viewer caches it too
        }
        self.file.seek(SeekFrom::Start(0))?;
        let v: Value = serde_json::from_reader(&mut self.file)?;
        self.parsed = Some(v.clone());
        Ok(v)
    }
}

/// Return (start,end) spans for elements inside a top-level JSON array.
/// Robust for strings/escapes and nested objects/arrays.
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

    // Helper to maybe start an element at the next non-ws
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
                    // We're at array level. ']' closes the array.
                    if b == b']' {
                        // Close last trailing element like: [ {...} ]
                        if let Some(s) = elem_start {
                            let end = prev_non_ws(bytes, i).unwrap_or(i) + 1;
                            spans.push((s, end as u64));
                            // elem_start = None;
                        }
                        break;
                    } else {
                        // '}' at depth 0 means element ends here (object at array level).
                        // This is okay only if elem started earlier.
                    }
                } else {
                    depth -= 1;
                }
            }
            b',' => {
                if depth == 0 {
                    // Element boundary at array level
                    if let Some(s) = elem_start.take() {
                        let end = prev_non_ws(bytes, i).unwrap_or(i);
                        spans.push((s, (end + 1) as u64));
                    }
                    want_new_elem = true;
                }
            }
            c if is_ws(c) => { /* skip */ }
            _ => {
                // Primitives (true/false/null/number) at array level
                if want_new_elem {
                    elem_start = Some(i as u64);
                    want_new_elem = false;
                }
            }
        }

        i += 1;
    }

    // Sanity: top-level array must have closed
    if bytes.get(i.saturating_sub(0)) != Some(&b']')
        && bytes.get(i.saturating_sub(1)) != Some(&b'\n')
    {
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

/* ---------------- Public entry point ---------------- */

pub fn load_file_auto(path: &Path) -> Result<(DetectedFileType, LazyJsonFile)> {
    let detected = sniff_file_type(path)?;
    let lazy = match detected {
        DetectedFileType::Ndjson => LazyJsonFile::Ndjson(NdjsonFile::open(path)?),
        DetectedFileType::JsonArray => LazyJsonFile::JsonArray(JsonArrayFile::open(path)?),
        DetectedFileType::JsonObject => LazyJsonFile::Single(SingleValueFile::open(path)?),
    };
    Ok((detected, lazy))
}

// pub fn load_file_lazy(path: &Path, file_type: &FileType) -> Result<LazyJsonFile> {
//     match file_type {
//         FileType::Ndjson => Ok(LazyJsonFile::Ndjson(NdjsonFile::open(path)?)),
//         FileType::Json => Ok(LazyJsonFile::JsonArray(JsonArrayFile::open(path)?)),
//     }
// }
