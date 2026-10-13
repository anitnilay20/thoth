use crate::error::{Result, ThothError};
use crate::file::loaders::FileLoader;
use crate::platform::FileIO;
use anyhow::Context;
use serde_json::Value;
use std::{fs::File, io::Read, path::Path};

/// Lazy loader for JSON files containing a top-level array
///
/// This loader indexes array element boundaries during initialization,
/// allowing for efficient random access to individual elements without
/// parsing the entire array.
pub struct JsonArrayFile {
    file: File,
    element_spans: Vec<(u64, u64)>, // (start, end) exclusive
}

impl JsonArrayFile {
    /// Open a JSON array file and index all element boundaries
    ///
    /// This reads the entire file to build an index of element spans,
    /// which allows for efficient random access later without re-parsing.
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path).with_context(|| "open JSON")?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        // Find the top-level array and index each element span without parsing it.
        let spans =
            index_json_array_elements(&buf).map_err(|e| ThothError::InvalidJsonStructure {
                reason: format!("failed to index top-level array: {}", e),
            })?;

        // Keep the file for later slice reads
        let file = File::open(path)?;
        Ok(Self {
            file,
            element_spans: spans,
        })
    }

    /// Returns the number of elements in the JSON array
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.element_spans.len()
    }

    /// Get a parsed JSON value at the specified array index
    ///
    /// This performs a position-independent read and is safe for parallel access.
    pub fn get(&mut self, idx: usize) -> Result<Value> {
        let (start, end) =
            *self
                .element_spans
                .get(idx)
                .ok_or_else(|| ThothError::InvalidJsonStructure {
                    reason: format!("JSON array element index {} out of bounds", idx),
                })?;
        let len = (end - start) as usize;
        let mut buf = vec![0u8; len];
        self.file.read_at(&mut buf, start)?;

        let v: Value = serde_json::from_slice(&buf)
            .with_context(|| format!("invalid element at index {}", idx))?;
        Ok(v)
    }

    /// Get raw bytes for an array element at the specified index
    ///
    /// This performs a position-independent read and is safe for parallel access.
    pub fn raw_element(&self, idx: usize) -> Result<Vec<u8>> {
        let (start, end) =
            *self
                .element_spans
                .get(idx)
                .ok_or_else(|| ThothError::InvalidJsonStructure {
                    reason: format!("Element index {} out of bounds", idx),
                })?;
        let len = (end - start) as usize;
        let mut buf = vec![0u8; len];
        self.file.read_at(&mut buf, start)?;

        Ok(buf)
    }
}

/// Index the boundaries of elements in a top-level JSON array
///
/// This function scans through a JSON array without fully parsing it,
/// recording the byte positions where each element starts and ends.
/// This enables efficient random access to array elements.
fn index_json_array_elements(bytes: &[u8]) -> Result<Vec<(u64, u64)>> {
    // Skip leading whitespace
    let mut i = skip_ws(bytes, 0).ok_or_else(|| ThothError::InvalidJsonStructure {
        reason: "empty file".to_string(),
    })?;
    if bytes.get(i) != Some(&b'[') {
        return Err(ThothError::InvalidJsonStructure {
            reason: "expected a top-level JSON array".to_string(),
        });
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
        return Err(ThothError::InvalidJsonStructure {
            reason: "unterminated top-level array".to_string(),
        });
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

impl FileLoader for JsonArrayFile {
    type Item = Value;

    fn open(path: &Path) -> Result<Self> {
        JsonArrayFile::open(path)
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn get(&mut self, idx: usize) -> Result<Self::Item> {
        self.get(idx)
    }

    fn raw_bytes(&self, idx: usize) -> Result<Vec<u8>> {
        self.raw_element(idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_json_array_basic_loading() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"[{{"id":1}},{{"id":2}},{{"id":3}}]"#).unwrap();
        file.flush().unwrap();

        let mut loader = JsonArrayFile::open(file.path()).unwrap();
        assert_eq!(loader.len(), 3);

        let val = loader.get(0).unwrap();
        assert_eq!(val["id"], 1);

        let val = loader.get(2).unwrap();
        assert_eq!(val["id"], 3);
    }

    #[test]
    fn test_json_array_empty() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "[]").unwrap();
        file.flush().unwrap();

        let loader = JsonArrayFile::open(file.path()).unwrap();
        assert_eq!(loader.len(), 0);
    }

    #[test]
    fn test_json_array_single_element() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"[{{"id":1}}]"#).unwrap();
        file.flush().unwrap();

        let mut loader = JsonArrayFile::open(file.path()).unwrap();
        assert_eq!(loader.len(), 1);

        let val = loader.get(0).unwrap();
        assert_eq!(val["id"], 1);
    }

    #[test]
    fn test_json_array_nested() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            r#"[{{"user":{{"name":"Alice"}}}},{{"user":{{"name":"Bob"}}}}]"#
        )
        .unwrap();
        file.flush().unwrap();

        let mut loader = JsonArrayFile::open(file.path()).unwrap();
        assert_eq!(loader.len(), 2);

        let val = loader.get(0).unwrap();
        assert_eq!(val["user"]["name"], "Alice");
    }

    #[test]
    fn test_json_array_with_whitespace() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            r#"[
                {{"id": 1}},
                {{"id": 2}},
                {{"id": 3}}
            ]"#
        )
        .unwrap();
        file.flush().unwrap();

        let mut loader = JsonArrayFile::open(file.path()).unwrap();
        assert_eq!(loader.len(), 3);

        let val = loader.get(1).unwrap();
        assert_eq!(val["id"], 2);
    }

    #[test]
    fn test_json_array_out_of_bounds() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"[{{"id":1}}]"#).unwrap();
        file.flush().unwrap();

        let mut loader = JsonArrayFile::open(file.path()).unwrap();
        assert!(loader.get(1).is_err());
        assert!(loader.get(100).is_err());
    }

    #[test]
    fn test_json_array_raw_bytes() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"[{{"id":1}},{{"id":2}}]"#).unwrap();
        file.flush().unwrap();

        let loader = JsonArrayFile::open(file.path()).unwrap();
        let raw = loader.raw_element(0).unwrap();
        let s = String::from_utf8(raw).unwrap();
        assert!(s.contains("\"id\""));
        assert!(s.contains("1"));
    }

    #[test]
    fn test_json_array_fileloader_trait() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"[{{"id":1}},{{"id":2}}]"#).unwrap();
        file.flush().unwrap();

        let mut loader: Box<dyn FileLoader<Item = Value>> =
            Box::new(JsonArrayFile::open(file.path()).unwrap());

        assert_eq!(loader.len(), 2);
        assert!(!loader.is_empty());

        let val = loader.get(0).unwrap();
        assert_eq!(val["id"], 1);
    }
}
