use crate::error::{Result, ThothError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::constants::{DEFAULT_SIDEBAR_WIDTH, MAX_RECENT_FILES, MIN_SIDEBAR_WIDTH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentState {
    #[serde(default)]
    recent_files: Vec<String>,
    #[serde(default = "default_sidebar_width")]
    sidebar_width: f32,
}

fn default_sidebar_width() -> f32 {
    DEFAULT_SIDEBAR_WIDTH
}

impl Default for PersistentState {
    fn default() -> Self {
        // Try to load from disk, fallback to empty state on error
        Self::load().unwrap_or(Self {
            recent_files: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
        })
    }
}

impl PersistentState {
    /// Get the path to the app state storage
    /// Returns: ~/.config/thoth/persistent_state.json on Linux/macOS
    ///          %APPDATA%/thoth/persistent_state.json on Windows
    fn storage_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().ok_or_else(|| ThothError::StateError {
            reason: "Failed to get config directory".to_string(),
        })?;
        let thoth_config_dir = config_dir.join("thoth");

        // Create directory if it doesn't exist
        if !thoth_config_dir.exists() {
            std::fs::create_dir_all(&thoth_config_dir).map_err(|e| ThothError::StateError {
                reason: format!("Failed to create thoth config directory: {}", e),
            })?;
        }

        Ok(thoth_config_dir.join("persistent_state.json"))
    }

    /// Load app state from disk
    pub fn load() -> Result<Self> {
        let path = Self::storage_path()?;

        if path.exists() {
            let contents = std::fs::read_to_string(&path).map_err(|e| ThothError::StateError {
                reason: format!("Failed to read app state: {}", e),
            })?;
            let app_state: PersistentState =
                serde_json::from_str(&contents).map_err(|e| ThothError::StateError {
                    reason: format!("Failed to parse app state: {}", e),
                })?;
            Ok(app_state)
        } else {
            // Try to migrate from old recent_files.json
            Self::migrate_from_old_format()
        }
    }

    /// Migrate from old recent_files.json format
    fn migrate_from_old_format() -> Result<Self> {
        let config_dir = dirs::config_dir().ok_or_else(|| ThothError::StateError {
            reason: "Failed to get config directory".to_string(),
        })?;
        let old_path = config_dir.join("thoth").join("recent_files.json");

        if old_path.exists() {
            // Read old format
            let contents =
                std::fs::read_to_string(&old_path).map_err(|e| ThothError::StateError {
                    reason: format!("Failed to read old recent files: {}", e),
                })?;

            #[derive(Deserialize)]
            struct OldFormat {
                files: Vec<String>,
            }

            if let Ok(old_data) = serde_json::from_str::<OldFormat>(&contents) {
                eprintln!("Migrating from old recent_files.json format...");
                let new_state = PersistentState {
                    recent_files: old_data.files,
                    sidebar_width: DEFAULT_SIDEBAR_WIDTH,
                };

                // Save in new format
                if new_state.save().is_ok() {
                    // Remove old file
                    let _ = std::fs::remove_file(&old_path);
                    eprintln!("Migration successful!");
                }

                return Ok(new_state);
            }
        }

        // No migration needed or failed, return default
        Ok(Self {
            recent_files: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
        })
    }

    /// Save app state to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::storage_path()?;
        let json = serde_json::to_string_pretty(self).map_err(|e| ThothError::StateError {
            reason: format!("Failed to serialize app state: {}", e),
        })?;
        std::fs::write(&path, &json).map_err(|e| ThothError::FileWriteError {
            path: path.clone(),
            reason: e.to_string(),
        })?;
        Ok(())
    }

    // Recent Files methods

    /// Add a file to recent files (moves to top if already exists)
    pub fn add_recent_file(&mut self, file_path: String) {
        // Remove if already exists
        self.recent_files.retain(|f| f != &file_path);

        // Add to front
        self.recent_files.insert(0, file_path);

        // Limit to MAX_RECENT_FILES
        if self.recent_files.len() > MAX_RECENT_FILES {
            self.recent_files.truncate(MAX_RECENT_FILES);
        }
    }

    /// Remove a file from recent files
    pub fn remove_recent_file(&mut self, file_path: &str) {
        self.recent_files.retain(|f| f != file_path);
    }

    /// Get all recent files
    pub fn get_recent_files(&self) -> &[String] {
        &self.recent_files
    }

    // Sidebar width methods

    /// Set the sidebar width
    pub fn set_sidebar_width(&mut self, width: f32) {
        self.sidebar_width = width.max(MIN_SIDEBAR_WIDTH); // Ensure minimum width
    }

    /// Get the sidebar width
    pub fn get_sidebar_width(&self) -> f32 {
        self.sidebar_width
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_recent_file() {
        let mut state = PersistentState {
            recent_files: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
        };
        state.add_recent_file("file1.json".to_string());
        state.add_recent_file("file2.json".to_string());

        assert_eq!(state.get_recent_files().len(), 2);
        assert_eq!(state.get_recent_files()[0], "file2.json");
        assert_eq!(state.get_recent_files()[1], "file1.json");
    }

    #[test]
    fn test_add_duplicate_moves_to_top() {
        let mut state = PersistentState {
            recent_files: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
        };
        state.add_recent_file("file1.json".to_string());
        state.add_recent_file("file2.json".to_string());
        state.add_recent_file("file1.json".to_string());

        assert_eq!(state.get_recent_files().len(), 2);
        assert_eq!(state.get_recent_files()[0], "file1.json");
        assert_eq!(state.get_recent_files()[1], "file2.json");
    }

    #[test]
    fn test_max_recent_files() {
        let mut state = PersistentState {
            recent_files: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
        };
        for i in 0..15 {
            state.add_recent_file(format!("file{}.json", i));
        }

        assert_eq!(state.get_recent_files().len(), MAX_RECENT_FILES);
        assert_eq!(state.get_recent_files()[0], "file14.json");
    }

    #[test]
    fn test_remove_recent_file() {
        let mut state = PersistentState {
            recent_files: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
        };
        state.add_recent_file("file1.json".to_string());
        state.add_recent_file("file2.json".to_string());
        state.remove_recent_file("file1.json");

        assert_eq!(state.get_recent_files().len(), 1);
        assert_eq!(state.get_recent_files()[0], "file2.json");
    }

    #[test]
    fn test_sidebar_width() {
        let mut state = PersistentState::default();

        assert_eq!(state.get_sidebar_width(), DEFAULT_SIDEBAR_WIDTH);

        state.set_sidebar_width(350.0);
        assert_eq!(state.get_sidebar_width(), 350.0);

        // Test minimum width enforcement
        state.set_sidebar_width(100.0);
        assert_eq!(state.get_sidebar_width(), MIN_SIDEBAR_WIDTH);
    }
}
