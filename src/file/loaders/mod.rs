mod json_array;
mod ndjson;
mod single;

pub use json_array::JsonArrayFile;
pub use ndjson::NdjsonFile;
pub use single::SingleValueFile;

use crate::error::Result;
use crate::file::detect_file_type::DetectedFileType;
use serde_json::Value;
use std::path::Path;

/// Common trait for all lazy file loaders
///
/// This trait defines the interface that all file loaders must implement,
/// regardless of format (JSON, CSV, TOML, YAML, etc.). It ensures consistent
/// behavior across different file formats.
///
/// # Design Philosophy
/// - Loaders should perform minimal work during `open()` - just enough to index the file
/// - Actual parsing happens lazily on `get()` calls
/// - All read operations should be position-independent (safe for parallel access)
/// - Raw bytes can be accessed without parsing
///
/// # Example: Implementing a new loader
/// ```rust,ignore
/// // Future CSV loader example
/// pub struct CsvFile {
///     file: File,
///     row_spans: Vec<(u64, u64)>,
/// }
///
/// impl FileLoader for CsvFile {
///     type Item = csv::StringRecord; // Or custom type
///
///     fn open(path: &Path) -> Result<Self> {
///         // Index CSV rows during open
///     }
///
///     fn len(&self) -> usize {
///         self.row_spans.len()
///     }
///
///     fn get(&mut self, idx: usize) -> Result<Self::Item> {
///         // Parse CSV row at index
///     }
///
///     fn raw_bytes(&self, idx: usize) -> Result<Vec<u8>> {
///         // Return raw row bytes
///     }
/// }
/// ```
#[allow(dead_code)]
pub trait FileLoader {
    /// The parsed data type this loader produces (e.g., serde_json::Value, csv::Row, etc.)
    type Item;

    /// Open a file and prepare it for lazy loading
    ///
    /// This should perform any necessary indexing or preparation work,
    /// but should not load or parse the entire file into memory.
    fn open(path: &Path) -> Result<Self>
    where
        Self: Sized;

    /// Returns the number of top-level elements/records in the file
    fn len(&self) -> usize;

    /// Returns true if the file contains no elements
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get a parsed item at the specified index
    ///
    /// This method should perform position-independent reads and be safe
    /// for parallel access (where the underlying format allows it).
    fn get(&mut self, idx: usize) -> Result<Self::Item>;

    /// Get raw bytes for an element at the specified index
    ///
    /// This method should perform position-independent reads and be safe
    /// for parallel access. Returns the raw bytes without parsing.
    fn raw_bytes(&self, idx: usize) -> Result<Vec<u8>>;
}

/// File type enumeration for JSON files
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

/// Unified interface for lazy JSON file loading
///
/// This enum dispatches to specific file loader implementations based on
/// the detected file type (NDJSON, JSON array, or single JSON object).
pub enum LazyJsonFile {
    Ndjson(NdjsonFile),
    JsonArray(JsonArrayFile),
    Single(SingleValueFile),
}

impl LazyJsonFile {
    /// Returns the number of top-level elements in the file
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match self {
            LazyJsonFile::Ndjson(f) => f.len(),
            LazyJsonFile::JsonArray(f) => f.len(),
            LazyJsonFile::Single(_) => 1,
        }
    }

    /// Get a parsed JSON value at the specified index
    pub fn get(&mut self, idx: usize) -> Result<Value> {
        match self {
            LazyJsonFile::Ndjson(f) => f.get(idx),
            LazyJsonFile::JsonArray(f) => f.get(idx),
            LazyJsonFile::Single(f) => f.get(idx),
        }
    }

    /// Get the raw bytes for an element at the specified index
    pub fn raw_slice(&self, idx: usize) -> Result<Vec<u8>> {
        match self {
            LazyJsonFile::Ndjson(f) => f.raw_line(idx),
            LazyJsonFile::JsonArray(f) => f.raw_element(idx),
            LazyJsonFile::Single(f) => f.raw_all(),
        }
    }
}

impl FileLoader for LazyJsonFile {
    type Item = Value;

    fn open(path: &Path) -> Result<Self> {
        let (_detected, lazy) = load_file_auto(path)?;
        Ok(lazy)
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn get(&mut self, idx: usize) -> Result<Self::Item> {
        self.get(idx)
    }

    fn raw_bytes(&self, idx: usize) -> Result<Vec<u8>> {
        self.raw_slice(idx)
    }
}

/// Load a JSON file with automatic format detection
///
/// This function detects the file type (NDJSON, JSON array, or single JSON object)
/// and returns the appropriate lazy loader.
pub fn load_file_auto(path: &Path) -> Result<(DetectedFileType, LazyJsonFile)> {
    use crate::file::detect_file_type::sniff_file_type;

    let detected = sniff_file_type(path)?;
    let lazy = match detected {
        DetectedFileType::Ndjson => LazyJsonFile::Ndjson(NdjsonFile::open(path)?),
        DetectedFileType::JsonArray => LazyJsonFile::JsonArray(JsonArrayFile::open(path)?),
        DetectedFileType::JsonObject => LazyJsonFile::Single(SingleValueFile::open(path)?),
    };
    Ok((detected, lazy))
}
