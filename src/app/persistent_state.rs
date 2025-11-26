use crate::error::{Result, ThothError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::constants::{DEFAULT_SIDEBAR_WIDTH, MAX_RECENT_FILES, MIN_SIDEBAR_WIDTH};

const MAX_SEARCH_HISTORY_PER_FILE: usize = 10;
const MAX_FILES_WITH_HISTORY: usize = 20; // Keep history for at most 20 files

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchHistoryStore {
    /// Maps file path to (last_accessed_timestamp, queries)
    histories: HashMap<String, (u64, Vec<String>)>,
}

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

    // Search history methods (single file with LRU for most recently used files)

    /// Get the path to the search history storage file
    fn search_history_storage_path() -> Result<PathBuf> {
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

        Ok(thoth_config_dir.join("search_history.json"))
    }

    /// Load all search history
    fn load_history_store() -> Result<SearchHistoryStore> {
        let path = Self::search_history_storage_path()?;

        if path.exists() {
            let contents = std::fs::read_to_string(&path).map_err(|e| ThothError::StateError {
                reason: format!("Failed to read search history: {}", e),
            })?;
            let store: SearchHistoryStore =
                serde_json::from_str(&contents).map_err(|e| ThothError::StateError {
                    reason: format!("Failed to parse search history: {}", e),
                })?;
            Ok(store)
        } else {
            Ok(SearchHistoryStore {
                histories: HashMap::new(),
            })
        }
    }

    /// Save all search history
    fn save_history_store(store: &SearchHistoryStore) -> Result<()> {
        let path = Self::search_history_storage_path()?;
        let json = serde_json::to_string_pretty(store).map_err(|e| ThothError::StateError {
            reason: format!("Failed to serialize search history: {}", e),
        })?;
        std::fs::write(&path, &json).map_err(|e| ThothError::FileWriteError {
            path: path.clone(),
            reason: e.to_string(),
        })?;
        Ok(())
    }

    /// Get current timestamp in seconds
    fn current_timestamp() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Load search history for a specific file
    pub fn load_search_history(file_path: &str) -> Result<Vec<String>> {
        let store = Self::load_history_store()?;
        Ok(store
            .histories
            .get(file_path)
            .map(|(_, queries)| queries.clone())
            .unwrap_or_default())
    }

    /// Add a search query to history for a specific file
    pub fn add_search_query(file_path: &str, query: String) -> Result<()> {
        if query.trim().is_empty() {
            return Ok(());
        }

        let mut store = Self::load_history_store().unwrap_or_else(|_| SearchHistoryStore {
            histories: HashMap::new(),
        });

        // Get or create history for this file
        let (_, queries) = store
            .histories
            .entry(file_path.to_string())
            .or_insert_with(|| (Self::current_timestamp(), Vec::new()));

        // Remove if already exists
        queries.retain(|q| q != &query);

        // Add to front
        queries.insert(0, query);

        // Limit to MAX_SEARCH_HISTORY_PER_FILE
        if queries.len() > MAX_SEARCH_HISTORY_PER_FILE {
            queries.truncate(MAX_SEARCH_HISTORY_PER_FILE);
        }

        // Update timestamp
        store.histories.get_mut(file_path).unwrap().0 = Self::current_timestamp();

        // Clean up old entries if we have too many files
        if store.histories.len() > MAX_FILES_WITH_HISTORY {
            // Sort by timestamp and keep only the most recent files
            let mut entries: Vec<_> = store.histories.iter().collect();
            entries.sort_by_key(|(_, (timestamp, _))| std::cmp::Reverse(*timestamp));

            let to_keep: HashMap<_, _> = entries
                .into_iter()
                .take(MAX_FILES_WITH_HISTORY)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            store.histories = to_keep;
        }

        Self::save_history_store(&store)
    }

    /// Clear search history for a specific file
    pub fn clear_search_history(file_path: &str) -> Result<()> {
        let mut store = Self::load_history_store()?;
        store.histories.remove(file_path);
        Self::save_history_store(&store)
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
