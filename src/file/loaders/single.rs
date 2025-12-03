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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_single_value_basic() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"{{"id":1,"name":"Alice"}}"#).unwrap();
        file.flush().unwrap();

        let mut loader = SingleValueFile::open(file.path()).unwrap();
        assert_eq!(loader.len(), 1);

        let val = loader.get(0).unwrap();
        assert_eq!(val["id"], 1);
        assert_eq!(val["name"], "Alice");
    }

    #[test]
    fn test_single_value_caching() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"{{"id":1}}"#).unwrap();
        file.flush().unwrap();

        let mut loader = SingleValueFile::open(file.path()).unwrap();

        // First access should parse
        let val1 = loader.get(0).unwrap();
        assert_eq!(val1["id"], 1);

        // Second access should use cache
        let val2 = loader.get(0).unwrap();
        assert_eq!(val2["id"], 1);
    }

    #[test]
    fn test_single_value_nested() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"{{"user":{{"name":"Alice","age":30}}}}"#).unwrap();
        file.flush().unwrap();

        let mut loader = SingleValueFile::open(file.path()).unwrap();
        let val = loader.get(0).unwrap();
        assert_eq!(val["user"]["name"], "Alice");
        assert_eq!(val["user"]["age"], 30);
    }

    #[test]
    fn test_single_value_empty_object() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{{}}").unwrap();
        file.flush().unwrap();

        let mut loader = SingleValueFile::open(file.path()).unwrap();
        let val = loader.get(0).unwrap();
        assert!(val.is_object());
        assert_eq!(val.as_object().unwrap().len(), 0);
    }

    #[test]
    fn test_single_value_out_of_bounds() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"{{"id":1}}"#).unwrap();
        file.flush().unwrap();

        let mut loader = SingleValueFile::open(file.path()).unwrap();
        assert!(loader.get(1).is_err());
        assert!(loader.get(100).is_err());
    }

    #[test]
    fn test_single_value_raw_bytes() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"{{"id":1}}"#).unwrap();
        file.flush().unwrap();

        let loader = SingleValueFile::open(file.path()).unwrap();
        let raw = loader.raw_all().unwrap();
        let s = String::from_utf8(raw).unwrap();
        assert_eq!(s, r#"{"id":1}"#);
    }

    #[test]
    fn test_single_value_raw_bytes_out_of_bounds() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"{{"id":1}}"#).unwrap();
        file.flush().unwrap();

        let loader = SingleValueFile::open(file.path()).unwrap();
        assert!(loader.raw_bytes(1).is_err());
    }

    #[test]
    fn test_single_value_fileloader_trait() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, r#"{{"id":1}}"#).unwrap();
        file.flush().unwrap();

        let mut loader: Box<dyn FileLoader<Item = Value>> =
            Box::new(SingleValueFile::open(file.path()).unwrap());

        assert_eq!(loader.len(), 1);
        assert!(!loader.is_empty());

        let val = loader.get(0).unwrap();
        assert_eq!(val["id"], 1);
    }
}
