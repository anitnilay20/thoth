use super::types::ThothError;

/// Recovery action to take after an error
#[derive(Debug, Clone)]
pub enum RecoveryAction {
    /// Clear the error and continue (error has been handled)
    ClearError,
    /// Retry the failed operation
    Retry,
    /// Show error to user and wait for action
    ShowError,
    /// Reset to initial state
    Reset,
}

/// Error recovery strategies
pub struct ErrorRecovery;

impl ErrorRecovery {
    /// Determine the recovery action for a given error
    pub fn get_recovery_action(error: &ThothError) -> RecoveryAction {
        match error {
            // File errors
            ThothError::FileNotFound { .. } => RecoveryAction::ShowError,
            ThothError::FileReadError { .. } => RecoveryAction::ShowError,
            ThothError::FileWriteError { .. } => RecoveryAction::ShowError,
            ThothError::InvalidFileType { .. } => RecoveryAction::ShowError,

            // JSON errors - show and allow user to try different file
            ThothError::JsonParseError { .. } => RecoveryAction::ShowError,
            ThothError::InvalidJsonStructure { .. } => RecoveryAction::ShowError,

            // Search errors - can be cleared silently
            ThothError::SearchError { .. } => RecoveryAction::ClearError,

            // UI errors
            ThothError::UIRenderError { .. } => RecoveryAction::ShowError,
            ThothError::StateError { .. } => RecoveryAction::Reset,

            // Update errors - show and allow retry
            ThothError::UpdateCheckError { .. } => RecoveryAction::ShowError,
            ThothError::UpdateDownloadError { .. } => RecoveryAction::ShowError,
            ThothError::UpdateInstallError { .. } => RecoveryAction::ShowError,

            // Settings errors - use defaults
            ThothError::SettingsLoadError { .. } => RecoveryAction::ClearError,
            ThothError::SettingsSaveError { .. } => RecoveryAction::ShowError,

            // PATH registry errors - show and continue
            ThothError::PathRegistryError { .. } => RecoveryAction::ShowError,

            // Unknown errors
            ThothError::Unknown { .. } => RecoveryAction::ShowError,
        }
    }

    /// Get a recovery suggestion message for the user
    pub fn get_recovery_suggestion(error: &ThothError) -> Option<String> {
        match error {
            ThothError::FileNotFound { .. } => Some("Try opening a different file.".to_string()),
            ThothError::FileReadError { .. } => {
                Some("Check file permissions and try again.".to_string())
            }
            ThothError::InvalidFileType { .. } => {
                Some("Please select a JSON or NDJSON file.".to_string())
            }
            ThothError::JsonParseError { .. } => {
                Some("Check if the file contains valid JSON.".to_string())
            }
            ThothError::UpdateCheckError { .. } => {
                Some("Check your internet connection and try again later.".to_string())
            }
            ThothError::UpdateDownloadError { .. } => {
                Some("Check your internet connection and try again.".to_string())
            }
            ThothError::PathRegistryError { .. } => Some(
                "You may need administrator privileges or manually add Thoth to your PATH."
                    .to_string(),
            ),
            _ => None,
        }
    }
}
