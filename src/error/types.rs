use std::path::PathBuf;

/// Custom error type for the Thoth application
#[derive(Debug, Clone)]
pub enum ThothError {
    // File-related errors
    FileNotFound { path: PathBuf },
    FileReadError { path: PathBuf, reason: String },
    FileWriteError { path: PathBuf, reason: String },
    FileParseError { path: PathBuf, reason: String },
    InvalidFileType { path: PathBuf, expected: String },

    // JSON/NDJSON parsing errors
    JsonParseError { line: Option<usize>, reason: String },
    InvalidJsonStructure { reason: String },

    // Search-related errors
    SearchError { query: String, reason: String },
    InvalidSearchPattern { pattern: String, reason: String },

    // UI-related errors
    UIRenderError { component: String, reason: String },
    StateError { reason: String },

    // Update-related errors
    UpdateCheckError { reason: String },
    UpdateDownloadError { version: String, reason: String },
    UpdateInstallError { reason: String },

    // Settings errors
    SettingsLoadError { reason: String },
    SettingsSaveError { reason: String },

    // Generic/unknown errors
    Unknown { message: String },
}

impl std::fmt::Display for ThothError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // File errors
            ThothError::FileNotFound { path } => {
                write!(f, "File not found: {}", path.display())
            }
            ThothError::FileReadError { path, reason } => {
                write!(f, "Failed to read file '{}': {}", path.display(), reason)
            }
            ThothError::FileWriteError { path, reason } => {
                write!(f, "Failed to write file '{}': {}", path.display(), reason)
            }
            ThothError::FileParseError { path, reason } => {
                write!(f, "Failed to parse file '{}': {}", path.display(), reason)
            }
            ThothError::InvalidFileType { path, expected } => {
                write!(
                    f,
                    "Invalid file type for '{}'. Expected: {}",
                    path.display(),
                    expected
                )
            }

            // JSON errors
            ThothError::JsonParseError { line, reason } => {
                if let Some(line) = line {
                    write!(f, "JSON parse error at line {}: {}", line, reason)
                } else {
                    write!(f, "JSON parse error: {}", reason)
                }
            }
            ThothError::InvalidJsonStructure { reason } => {
                write!(f, "Invalid JSON structure: {}", reason)
            }

            // Search errors
            ThothError::SearchError { query, reason } => {
                write!(f, "Search failed for '{}': {}", query, reason)
            }
            ThothError::InvalidSearchPattern { pattern, reason } => {
                write!(f, "Invalid search pattern '{}': {}", pattern, reason)
            }

            // UI errors
            ThothError::UIRenderError { component, reason } => {
                write!(f, "Failed to render {}: {}", component, reason)
            }
            ThothError::StateError { reason } => {
                write!(f, "State error: {}", reason)
            }

            // Update errors
            ThothError::UpdateCheckError { reason } => {
                write!(f, "Failed to check for updates: {}", reason)
            }
            ThothError::UpdateDownloadError { version, reason } => {
                write!(f, "Failed to download update {}: {}", version, reason)
            }
            ThothError::UpdateInstallError { reason } => {
                write!(f, "Failed to install update: {}", reason)
            }

            // Settings errors
            ThothError::SettingsLoadError { reason } => {
                write!(f, "Failed to load settings: {}", reason)
            }
            ThothError::SettingsSaveError { reason } => {
                write!(f, "Failed to save settings: {}", reason)
            }

            // Generic
            ThothError::Unknown { message } => {
                write!(f, "An error occurred: {}", message)
            }
        }
    }
}

impl std::error::Error for ThothError {}

// Convenience conversions from common error types
impl From<std::io::Error> for ThothError {
    fn from(err: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match err.kind() {
            ErrorKind::NotFound => ThothError::FileNotFound {
                path: PathBuf::new(),
            },
            ErrorKind::PermissionDenied => ThothError::Unknown {
                message: format!("Permission denied: {}", err),
            },
            _ => ThothError::Unknown {
                message: err.to_string(),
            },
        }
    }
}

impl From<serde_json::Error> for ThothError {
    fn from(err: serde_json::Error) -> Self {
        ThothError::JsonParseError {
            line: err.line().into(),
            reason: err.to_string(),
        }
    }
}

impl From<String> for ThothError {
    fn from(message: String) -> Self {
        ThothError::Unknown { message }
    }
}

impl From<&str> for ThothError {
    fn from(message: &str) -> Self {
        ThothError::Unknown {
            message: message.to_string(),
        }
    }
}

impl From<anyhow::Error> for ThothError {
    fn from(err: anyhow::Error) -> Self {
        ThothError::Unknown {
            message: err.to_string(),
        }
    }
}

impl From<reqwest::Error> for ThothError {
    fn from(err: reqwest::Error) -> Self {
        ThothError::UpdateCheckError {
            reason: err.to_string(),
        }
    }
}
