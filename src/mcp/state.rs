//! Server state — manages open files for the MCP server.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::error::Result;
use crate::file::detect_file_type::DetectedFileType;
use crate::file::loaders::{FileKind, FileType, load_file_auto};

/// Represents a single file opened by the MCP server.
pub struct OpenFile {
    pub path: PathBuf,
    pub detected_type: DetectedFileType,
    pub file_type: FileType,
    pub file_kind: FileKind,
}

impl OpenFile {
    /// Open a file at the given path with automatic format detection.
    pub fn open(path: &Path) -> Result<Self> {
        let (detected, file_type) = load_file_auto(path)?;
        let file_kind = FileKind::from(detected);
        Ok(Self {
            path: path.to_path_buf(),
            detected_type: detected,
            file_type,
            file_kind,
        })
    }

    /// Return the number of top-level records.
    pub fn record_count(&self) -> usize {
        self.file_type.len()
    }

    /// Return the detected file type as a human-readable string.
    pub fn type_name(&self) -> &'static str {
        match self.detected_type {
            DetectedFileType::Ndjson => "ndjson",
            DetectedFileType::JsonArray => "json_array",
            DetectedFileType::JsonObject => "json_object",
        }
    }
}

/// Thread-safe shared state for the MCP server.
///
/// Keyed by a user-chosen handle (defaults to the file path string).
#[derive(Clone, Default)]
pub struct ServerState {
    inner: Arc<Mutex<ServerStateInner>>,
}

#[derive(Default)]
struct ServerStateInner {
    files: HashMap<String, OpenFile>,
    next_id: u64,
}

impl ServerState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a file and return its handle.
    pub fn open_file(&self, path: &Path) -> Result<(String, FileInfo)> {
        let open = OpenFile::open(path)?;
        let info = FileInfo {
            handle: String::new(), // filled below
            path: path.display().to_string(),
            file_type: open.type_name().to_string(),
            record_count: open.record_count(),
        };

        let mut inner = self.inner.lock().unwrap();
        inner.next_id += 1;
        let handle = format!("file_{}", inner.next_id);
        let info = FileInfo {
            handle: handle.clone(),
            ..info
        };
        inner.files.insert(handle.clone(), open);
        Ok((handle, info))
    }

    /// Close a file by handle. Returns true if the file was found and removed.
    pub fn close_file(&self, handle: &str) -> bool {
        let mut inner = self.inner.lock().unwrap();
        inner.files.remove(handle).is_some()
    }

    /// Run a closure with mutable access to an open file.
    pub fn with_file<F, T>(&self, handle: &str, f: F) -> Option<T>
    where
        F: FnOnce(&mut OpenFile) -> T,
    {
        let mut inner = self.inner.lock().unwrap();
        inner.files.get_mut(handle).map(f)
    }

    /// Get info about an open file.
    pub fn file_info(&self, handle: &str) -> Option<FileInfo> {
        let inner = self.inner.lock().unwrap();
        inner.files.get(handle).map(|f| FileInfo {
            handle: handle.to_string(),
            path: f.path.display().to_string(),
            file_type: f.type_name().to_string(),
            record_count: f.record_count(),
        })
    }

    /// List all open file handles.
    #[allow(dead_code)]
    pub fn list_handles(&self) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        inner.files.keys().cloned().collect()
    }
}

/// Serializable file metadata returned by several tools.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileInfo {
    pub handle: String,
    pub path: String,
    pub file_type: String,
    pub record_count: usize,
}
