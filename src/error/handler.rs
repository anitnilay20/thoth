use super::types::ThothError;

/// Centralized error handler for consistent error display across the application
pub struct ErrorHandler;

impl ErrorHandler {
    /// Get a user-friendly error message for display in the UI
    pub fn get_user_message(error: &ThothError) -> String {
        match error {
            // File errors - user-friendly messages
            ThothError::FileNotFound { path } => {
                format!("Could not find the file:\n{}", path.display())
            }
            ThothError::FileReadError { path, .. } => {
                format!(
                    "Unable to read the file:\n{}\n\nPlease check if the file exists and you have permission to read it.",
                    path.display()
                )
            }
            ThothError::FileWriteError { path, .. } => {
                format!(
                    "Unable to write to the file:\n{}\n\nPlease check if you have permission to write to this location.",
                    path.display()
                )
            }
            ThothError::FileParseError { path, reason } => {
                format!(
                    "The file could not be parsed:\n{}\n\nReason: {}",
                    path.display(),
                    reason
                )
            }
            ThothError::InvalidFileType { path, expected } => {
                format!(
                    "Invalid file type:\n{}\n\nExpected: {}",
                    path.display(),
                    expected
                )
            }

            // JSON errors
            ThothError::JsonParseError { line, reason } => {
                if let Some(line) = line {
                    format!("Invalid JSON at line {}:\n{}", line, reason)
                } else {
                    format!("Invalid JSON:\n{}", reason)
                }
            }
            ThothError::InvalidJsonStructure { reason } => {
                format!("The JSON structure is not valid:\n{}", reason)
            }

            // Search errors
            ThothError::SearchError { query, reason } => {
                format!("Search failed for '{}':\n{}", query, reason)
            }

            // UI errors
            ThothError::UIRenderError { component, reason } => {
                format!("Display error in {}:\n{}", component, reason)
            }
            ThothError::StateError { reason } => {
                format!("Application state error:\n{}", reason)
            }

            // Update errors
            ThothError::UpdateCheckError { reason } => {
                format!(
                    "Could not check for updates:\n{}\n\nPlease check your internet connection.",
                    reason
                )
            }
            ThothError::UpdateDownloadError { version, reason } => {
                format!("Failed to download version {}:\n{}", version, reason)
            }
            ThothError::UpdateInstallError { reason } => {
                format!("Failed to install update:\n{}", reason)
            }

            // Settings errors
            ThothError::SettingsLoadError { reason } => {
                format!(
                    "Could not load settings:\n{}\n\nDefault settings will be used.",
                    reason
                )
            }
            ThothError::SettingsSaveError { reason } => {
                format!("Could not save settings:\n{}", reason)
            }

            // Generic
            ThothError::Unknown { message } => {
                format!("An unexpected error occurred:\n{}", message)
            }
        }
    }

    /// Get a technical error message (for logs/debugging)
    pub fn get_technical_message(error: &ThothError) -> String {
        format!("{:?}", error)
    }

    /// Determine if an error is recoverable
    pub fn is_recoverable(error: &ThothError) -> bool {
        match error {
            // File errors - mostly recoverable (user can select different file)
            ThothError::FileNotFound { .. } => true,
            ThothError::FileReadError { .. } => true,
            ThothError::FileParseError { .. } => true,
            ThothError::InvalidFileType { .. } => true,
            ThothError::FileWriteError { .. } => false, // More serious

            // JSON errors - recoverable (user can try different file)
            ThothError::JsonParseError { .. } => true,
            ThothError::InvalidJsonStructure { .. } => true,

            // Search errors - always recoverable
            ThothError::SearchError { .. } => true,

            // UI errors - depends on severity
            ThothError::UIRenderError { .. } => true,
            ThothError::StateError { .. } => false,

            // Update errors - all recoverable
            ThothError::UpdateCheckError { .. } => true,
            ThothError::UpdateDownloadError { .. } => true,
            ThothError::UpdateInstallError { .. } => true,

            // Settings errors - recoverable
            ThothError::SettingsLoadError { .. } => true,
            ThothError::SettingsSaveError { .. } => true,

            // Unknown errors - assume not recoverable
            ThothError::Unknown { .. } => false,
        }
    }

    /// Log an error (for debugging)
    pub fn log_error(error: &ThothError) {
        eprintln!("[ERROR] {}", Self::get_technical_message(error));
    }
}
