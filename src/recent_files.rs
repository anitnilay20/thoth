use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const MAX_RECENT_FILES: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFiles {
    files: Vec<String>,
}

impl Default for RecentFiles {
    fn default() -> Self {
        // Try to load from disk, fallback to empty list on error
        Self::load().unwrap_or(Self { files: Vec::new() })
    }
}

impl RecentFiles {
    /// Get the path to the recent files storage
    /// Returns: ~/.config/thoth/recent_files.json on Linux/macOS
    ///          %APPDATA%/thoth/recent_files.json on Windows
    fn storage_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Failed to get config directory")?;
        let thoth_config_dir = config_dir.join("thoth");

        // Create directory if it doesn't exist
        if !thoth_config_dir.exists() {
            std::fs::create_dir_all(&thoth_config_dir)
                .context("Failed to create thoth config directory")?;
        }

        Ok(thoth_config_dir.join("recent_files.json"))
    }

    /// Load recent files from disk
    pub fn load() -> Result<Self> {
        let path = Self::storage_path()?;

        if path.exists() {
            let contents = std::fs::read_to_string(&path).context("Failed to read recent files")?;
            let recent_files: RecentFiles =
                serde_json::from_str(&contents).context("Failed to parse recent files")?;
            Ok(recent_files)
        } else {
            Ok(Self { files: Vec::new() })
        }
    }

    /// Save recent files to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::storage_path()?;
        let json =
            serde_json::to_string_pretty(self).context("Failed to serialize recent files")?;
        std::fs::write(&path, json).context("Failed to write recent files")?;
        Ok(())
    }

    /// Add a file to recent files (moves to top if already exists)
    pub fn add_file(&mut self, file_path: String) {
        // Remove if already exists
        self.files.retain(|f| f != &file_path);

        // Add to front
        self.files.insert(0, file_path);

        // Limit to MAX_RECENT_FILES
        if self.files.len() > MAX_RECENT_FILES {
            self.files.truncate(MAX_RECENT_FILES);
        }
    }

    /// Remove a file from recent files
    pub fn remove_file(&mut self, file_path: &str) {
        self.files.retain(|f| f != file_path);
    }

    /// Get all recent files
    pub fn get_files(&self) -> &[String] {
        &self.files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_file() {
        let mut recent = RecentFiles::default();
        recent.add_file("file1.json".to_string());
        recent.add_file("file2.json".to_string());

        assert_eq!(recent.get_files().len(), 2);
        assert_eq!(recent.get_files()[0], "file2.json");
        assert_eq!(recent.get_files()[1], "file1.json");
    }

    #[test]
    fn test_add_duplicate_moves_to_top() {
        let mut recent = RecentFiles::default();
        recent.add_file("file1.json".to_string());
        recent.add_file("file2.json".to_string());
        recent.add_file("file1.json".to_string());

        assert_eq!(recent.get_files().len(), 2);
        assert_eq!(recent.get_files()[0], "file1.json");
        assert_eq!(recent.get_files()[1], "file2.json");
    }

    #[test]
    fn test_max_recent_files() {
        let mut recent = RecentFiles::default();
        for i in 0..15 {
            recent.add_file(format!("file{}.json", i));
        }

        assert_eq!(recent.get_files().len(), MAX_RECENT_FILES);
        assert_eq!(recent.get_files()[0], "file14.json");
    }

    #[test]
    fn test_remove_file() {
        let mut recent = RecentFiles::default();
        recent.add_file("file1.json".to_string());
        recent.add_file("file2.json".to_string());
        recent.remove_file("file1.json");

        assert_eq!(recent.get_files().len(), 1);
        assert_eq!(recent.get_files()[0], "file2.json");
    }
}
