use std::path::PathBuf;

/// Custom error type for the Thoth application
#[derive(Debug, Clone, PartialEq)]
pub enum ThothError {
    // File-related errors
    FileNotFound {
        path: PathBuf,
    },
    FileReadError {
        path: PathBuf,
        reason: String,
    },
    FileWriteError {
        path: PathBuf,
        reason: String,
    },
    InvalidFileType {
        path: PathBuf,
        expected: String,
    },

    // JSON/NDJSON parsing errors
    JsonParseError {
        line: Option<usize>,
        reason: String,
    },
    InvalidJsonStructure {
        reason: String,
    },

    // Search-related errors
    SearchError {
        query: String,
        reason: String,
    },

    // UI-related errors
    UIRenderError {
        component: String,
        reason: String,
    },
    StateError {
        reason: String,
    },

    // Update-related errors
    UpdateCheckError {
        reason: String,
    },
    UpdateDownloadError {
        version: String,
        reason: String,
    },
    UpdateInstallError {
        reason: String,
    },

    // Settings errors
    SettingsLoadError {
        reason: String,
    },
    SettingsSaveError {
        reason: String,
    },

    // PATH registry errors
    PathRegistryError {
        reason: String,
    },

    // Generic/unknown errors
    Unknown {
        message: String,
    },

    // Plugin errors
    PluginDirectoryInvalid {
        dir: String,
    },
    PluginFileInvalid {
        path: PathBuf,
    },
    PluginLoadError {
        path: PathBuf,
        reason: String,
    },
    PluginDownloadError {
        name: String,
        url: String,
        reason: String,
    },
    PluginRemoveError {
        name: String,
        reason: String,
    },

    // Download/save errors
    DownloadError {
        url: String,
        reason: String,
    },
    FileSaveError {
        path: PathBuf,
        reason: String,
    },

    // Database errors
    DatabaseError {
        reason: String,
    },
    DatabaseConversionError {
        reason: String,
    },
    DatabaseQueryError {
        query: String,
        reason: String,
    },
    DatabaseParameterError {
        reason: String,
    },
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

            // PATH registry errors
            ThothError::PathRegistryError { reason } => {
                write!(f, "Failed to register PATH: {}", reason)
            }

            // Generic
            ThothError::Unknown { message } => {
                write!(f, "An error occurred: {}", message)
            }

            // Plugin
            ThothError::PluginDirectoryInvalid { dir } => {
                write!(f, "Unable to load from plugin directory: {}", dir)
            }

            ThothError::PluginFileInvalid { path } => {
                write!(f, "Invalid plugin file: {}", path.display())
            }

            ThothError::PluginLoadError { path, reason } => {
                write!(f, "Failed to load plugin '{}': {}", path.display(), reason)
            }
            ThothError::PluginDownloadError { name, url, reason } => {
                write!(
                    f,
                    "Failed to download plugin '{}' from '{}': {}",
                    name, url, reason
                )
            }
            ThothError::PluginRemoveError { name, reason } => {
                write!(f, "Failed to remove plugin '{}': {}", name, reason)
            }
            ThothError::DownloadError { url, reason } => {
                write!(f, "Failed to download '{}': {}", url, reason)
            }
            ThothError::FileSaveError { path, reason } => {
                write!(f, "Failed to save file '{}': {}", path.display(), reason)
            }

            // Database errors
            ThothError::DatabaseError { reason } => {
                write!(f, "Database error: {}", reason)
            }
            ThothError::DatabaseConversionError { reason } => {
                write!(f, "Database conversion error: {}", reason)
            }
            ThothError::DatabaseQueryError { query, reason } => {
                write!(f, "Database query error for '{}': {}", query, reason)
            }
            ThothError::DatabaseParameterError { reason } => {
                write!(f, "Database parameter error: {}", reason)
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
            ErrorKind::NotFound => ThothError::Unknown {
                message: format!("File not found: {}", err),
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

impl From<arrow::error::ArrowError> for ThothError {
    fn from(err: arrow::error::ArrowError) -> Self {
        ThothError::DatabaseConversionError {
            reason: err.to_string(),
        }
    }
}

impl From<duckdb::Error> for ThothError {
    fn from(value: duckdb::Error) -> Self {
        match value {
            duckdb::Error::DuckDBFailure(error, _reason) => ThothError::DatabaseError {
                reason: error.to_string(),
            },
            duckdb::Error::FromSqlConversionFailure(_, _, error) => {
                ThothError::DatabaseConversionError {
                    reason: error.to_string(),
                }
            }
            duckdb::Error::IntegralValueOutOfRange(_, _) => ThothError::DatabaseConversionError {
                reason: "Integral value out of range".to_string(),
            },
            duckdb::Error::UnsignedIntegralValueOutOfRange(_, _) => {
                ThothError::DatabaseConversionError {
                    reason: "Unsigned integral value out of range".to_string(),
                }
            }
            duckdb::Error::Utf8Error(utf8_error) => ThothError::DatabaseConversionError {
                reason: utf8_error.to_string(),
            },
            duckdb::Error::NulError(nul_error) => ThothError::DatabaseConversionError {
                reason: nul_error.to_string(),
            },
            duckdb::Error::InvalidParameterName(name) => ThothError::DatabaseParameterError {
                reason: format!("Invalid parameter name: {}", name),
            },
            duckdb::Error::InvalidPath(path_buf) => ThothError::FileReadError {
                path: path_buf,
                reason: "Invalid database path".to_string(),
            },
            duckdb::Error::ExecuteReturnedResults => ThothError::DatabaseError {
                reason: "Execute returned results unexpectedly".to_string(),
            },
            duckdb::Error::QueryReturnedNoRows => ThothError::DatabaseQueryError {
                query: "unknown".to_string(),
                reason: "Query returned no rows".to_string(),
            },
            duckdb::Error::QueryReturnedMoreThanOneRow => ThothError::DatabaseQueryError {
                query: "unknown".to_string(),
                reason: "Query returned more than one row".to_string(),
            },
            duckdb::Error::InvalidColumnIndex(idx) => ThothError::DatabaseQueryError {
                query: "unknown".to_string(),
                reason: format!("Invalid column index: {}", idx),
            },
            duckdb::Error::InvalidColumnName(name) => ThothError::DatabaseQueryError {
                query: "unknown".to_string(),
                reason: format!("Invalid column name: {}", name),
            },
            duckdb::Error::InvalidColumnType(idx, name, typ) => {
                ThothError::DatabaseConversionError {
                    reason: format!(
                        "Invalid column type for column {} ({}, {}): expected different type",
                        idx, name, typ
                    ),
                }
            }
            duckdb::Error::ArrowTypeToDuckdbType(_, data_type) => {
                ThothError::DatabaseConversionError {
                    reason: format!(
                        "Arrow type to DuckDB type conversion failed: {:?}",
                        data_type
                    ),
                }
            }
            duckdb::Error::StatementChangedRows(rows) => ThothError::DatabaseError {
                reason: format!("Statement changed {} rows unexpectedly", rows),
            },
            duckdb::Error::ToSqlConversionFailure(error) => ThothError::DatabaseConversionError {
                reason: error.to_string(),
            },
            duckdb::Error::InvalidQuery => ThothError::DatabaseQueryError {
                query: "unknown".to_string(),
                reason: "Invalid query".to_string(),
            },
            duckdb::Error::MultipleStatement => ThothError::DatabaseQueryError {
                query: "unknown".to_string(),
                reason: "Multiple statements not supported".to_string(),
            },
            duckdb::Error::InvalidParameterCount(expected, actual) => {
                ThothError::DatabaseParameterError {
                    reason: format!(
                        "Invalid parameter count: expected {}, got {}",
                        expected, actual
                    ),
                }
            }
            duckdb::Error::InvalidParameterIndex(idx) => ThothError::DatabaseParameterError {
                reason: format!("Invalid parameter index: {}", idx),
            },
            duckdb::Error::AppendError => ThothError::DatabaseError {
                reason: "Append error".to_string(),
            },
            _ => ThothError::DatabaseError {
                reason: value.to_string(),
            },
        }
    }
}
