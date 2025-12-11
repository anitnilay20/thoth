use crate::error::{Result, ThothError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::shortcuts::KeyboardShortcuts;
use crate::theme::Theme;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Configuration file version for migration support
    #[serde(default)]
    pub version: u32,

    /// Dark mode enabled
    pub dark_mode: bool,

    /// Font size for UI
    pub font_size: f32,

    /// Font family for UI
    #[serde(default)]
    pub font_family: Option<String>,

    /// Window settings for multi-window support
    pub window: WindowSettings,

    /// Auto-update settings
    pub updates: UpdateSettings,

    /// Keyboard shortcuts
    pub shortcuts: KeyboardShortcuts,

    /// Developer settings
    pub dev: DeveloperSettings,

    /// Theme color settings
    pub theme: Theme,

    /// Performance settings
    #[serde(default)]
    pub performance: PerformanceSettings,

    /// File viewer behavior settings
    #[serde(default)]
    pub viewer: ViewerSettings,

    /// UI preferences
    #[serde(default)]
    pub ui: UiSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DeveloperSettings {
    /// Show profiling UI (puffin/egui profiler)
    #[serde(default)]
    pub show_profiler: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowSettings {
    /// Default window width
    pub default_width: f32,

    /// Default window height
    pub default_height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateSettings {
    /// Check for updates automatically
    pub auto_check: bool,

    /// Update check interval in hours
    pub check_interval_hours: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceSettings {
    /// LRU cache size for parsed JSON values (default: 100)
    /// Higher values use more memory but improve performance when re-visiting nodes
    pub cache_size: usize,

    /// Maximum file size to load in MB (default: 500 MB)
    /// Files larger than this will show a warning
    pub max_file_size_mb: usize,

    /// Number of recent files to remember (default: 10)
    pub max_recent_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewerSettings {
    /// Auto-expand JSON tree depth on file open (default: 0 = collapsed)
    /// Set to 1 to expand root level, 2 for two levels, etc.
    pub auto_expand_depth: usize,

    /// Number of rows margin before triggering scroll (default: 3)
    pub scroll_margin: usize,

    /// Enable syntax highlighting in JSON viewer (default: true)
    pub syntax_highlighting: bool,

    /// Show line numbers in viewer (default: false)
    pub show_line_numbers: bool,

    /// Indent size for JSON tree (default: 16px)
    pub indent_size: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    /// Default sidebar width (default: 350px)
    pub sidebar_width: f32,

    /// Remember sidebar state across sessions (default: true)
    pub remember_sidebar_state: bool,

    /// Show status bar (default: true)
    pub show_status_bar: bool,

    /// Show toolbar (default: true)
    pub show_toolbar: bool,

    /// Enable animations (default: true)
    pub enable_animations: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            dark_mode: true,
            font_size: 14.0,
            font_family: None,
            window: WindowSettings::default(),
            updates: UpdateSettings::default(),
            shortcuts: KeyboardShortcuts::default(),
            dev: DeveloperSettings::default(),
            theme: Theme::default(),
            performance: PerformanceSettings::default(),
            viewer: ViewerSettings::default(),
            ui: UiSettings::default(),
        }
    }
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            default_width: 1200.0,
            default_height: 800.0,
        }
    }
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            auto_check: true,
            check_interval_hours: 24,
        }
    }
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            cache_size: 100,
            max_file_size_mb: 500,
            max_recent_files: 10,
        }
    }
}

impl Default for ViewerSettings {
    fn default() -> Self {
        Self {
            auto_expand_depth: 0,
            scroll_margin: 3,
            syntax_highlighting: true,
            show_line_numbers: false,
            indent_size: 16.0,
        }
    }
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            sidebar_width: 350.0,
            remember_sidebar_state: true,
            show_status_bar: true,
            show_toolbar: true,
            enable_animations: true,
        }
    }
}

impl Settings {
    /// Current configuration version
    pub const CURRENT_VERSION: u32 = 1;

    /// Get the path to the settings file
    /// Returns: ~/.config/thoth/settings.toml on Linux/macOS
    ///          %APPDATA%/thoth/settings.toml on Windows
    pub fn settings_file_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().ok_or_else(|| ThothError::SettingsLoadError {
            reason: "Failed to get config directory".to_string(),
        })?;

        let thoth_config_dir = config_dir.join("thoth");

        // Create directory if it doesn't exist
        if !thoth_config_dir.exists() {
            std::fs::create_dir_all(&thoth_config_dir).map_err(|e| {
                ThothError::SettingsSaveError {
                    reason: format!("Failed to create thoth config directory: {}", e),
                }
            })?;
        }

        Ok(thoth_config_dir.join("settings.toml"))
    }

    /// Validate settings and return user-friendly error messages
    pub fn validate(&self) -> Result<()> {
        // Validate font size
        if self.font_size < 8.0 || self.font_size > 72.0 {
            return Err(ThothError::SettingsLoadError {
                reason: format!(
                    "Invalid font_size: {}. Must be between 8.0 and 72.0",
                    self.font_size
                ),
            });
        }

        // Validate window dimensions
        if self.window.default_width < 400.0 || self.window.default_width > 7680.0 {
            return Err(ThothError::SettingsLoadError {
                reason: format!(
                    "Invalid window width: {}. Must be between 400.0 and 7680.0",
                    self.window.default_width
                ),
            });
        }

        if self.window.default_height < 300.0 || self.window.default_height > 4320.0 {
            return Err(ThothError::SettingsLoadError {
                reason: format!(
                    "Invalid window height: {}. Must be between 300.0 and 4320.0",
                    self.window.default_height
                ),
            });
        }

        // Validate performance settings
        if self.performance.cache_size == 0 {
            return Err(ThothError::SettingsLoadError {
                reason: "Invalid cache_size: 0. Must be at least 1".to_string(),
            });
        }

        if self.performance.cache_size > 10000 {
            return Err(ThothError::SettingsLoadError {
                reason: format!(
                    "Invalid cache_size: {}. Maximum is 10000 (recommended: 100-1000)",
                    self.performance.cache_size
                ),
            });
        }

        if self.performance.max_file_size_mb == 0 {
            return Err(ThothError::SettingsLoadError {
                reason: "Invalid max_file_size_mb: 0. Must be at least 1".to_string(),
            });
        }

        if self.performance.max_recent_files == 0 || self.performance.max_recent_files > 100 {
            return Err(ThothError::SettingsLoadError {
                reason: format!(
                    "Invalid max_recent_files: {}. Must be between 1 and 100",
                    self.performance.max_recent_files
                ),
            });
        }

        // Validate viewer settings
        if self.viewer.auto_expand_depth > 10 {
            return Err(ThothError::SettingsLoadError {
                reason: format!(
                    "Invalid auto_expand_depth: {}. Maximum is 10 (can cause performance issues)",
                    self.viewer.auto_expand_depth
                ),
            });
        }

        if self.viewer.indent_size < 4.0 || self.viewer.indent_size > 64.0 {
            return Err(ThothError::SettingsLoadError {
                reason: format!(
                    "Invalid indent_size: {}. Must be between 4.0 and 64.0",
                    self.viewer.indent_size
                ),
            });
        }

        // Validate UI settings
        if self.ui.sidebar_width < 200.0 || self.ui.sidebar_width > 1000.0 {
            return Err(ThothError::SettingsLoadError {
                reason: format!(
                    "Invalid sidebar_width: {}. Must be between 200.0 and 1000.0",
                    self.ui.sidebar_width
                ),
            });
        }

        // Validate update settings
        if self.updates.check_interval_hours == 0 {
            return Err(ThothError::SettingsLoadError {
                reason: "Invalid check_interval_hours: 0. Must be at least 1".to_string(),
            });
        }

        Ok(())
    }

    /// Migrate settings from older versions to current version
    fn migrate(&mut self) {
        // Currently at version 1, no migrations needed yet
        // This structure allows for future migrations:
        // if self.version < 2 {
        //     // Migrate from v1 to v2
        //     self.version = 2;
        // }
        // if self.version < 3 {
        //     // Migrate from v2 to v3
        //     self.version = 3;
        // }

        // Ensure version is current
        if self.version < Self::CURRENT_VERSION {
            self.version = Self::CURRENT_VERSION;
        }
    }

    /// Load settings from file, or create default if file doesn't exist
    pub fn load() -> Result<Self> {
        let settings_path = Self::settings_file_path()?;

        if settings_path.exists() {
            let contents = std::fs::read_to_string(&settings_path).map_err(|e| {
                ThothError::SettingsLoadError {
                    reason: format!("Failed to read settings file: {}", e),
                }
            })?;

            let mut settings: Settings =
                toml::from_str(&contents).map_err(|e| ThothError::SettingsLoadError {
                    reason: format!("Failed to parse settings file: {}", e),
                })?;

            // Migrate settings if needed
            settings.migrate();

            // Validate settings
            settings.validate()?;

            // Save settings back to file to ensure any new fields are added
            // This allows seamless updates when new settings are added to the struct
            settings.save()?;

            Ok(settings)
        } else {
            // Create default settings file
            let default_settings = Self::default();
            default_settings.save()?;
            Ok(default_settings)
        }
    }

    /// Save settings to file
    pub fn save(&self) -> Result<()> {
        let settings_path = Self::settings_file_path()?;

        let toml_string =
            toml::to_string_pretty(self).map_err(|e| ThothError::SettingsSaveError {
                reason: format!("Failed to serialize settings: {}", e),
            })?;

        std::fs::write(&settings_path, toml_string).map_err(|e| ThothError::SettingsSaveError {
            reason: format!("Failed to write settings file: {}", e),
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.version, 1);
        assert!(settings.dark_mode);
        assert_eq!(settings.font_size, 14.0);
        assert_eq!(settings.window.default_width, 1200.0);
        assert_eq!(settings.window.default_height, 800.0);
        assert_eq!(settings.performance.cache_size, 100);
        assert_eq!(settings.performance.max_file_size_mb, 500);
        assert_eq!(settings.viewer.auto_expand_depth, 0);
        assert!(settings.viewer.syntax_highlighting);
        assert_eq!(settings.ui.sidebar_width, 350.0);
    }

    #[test]
    fn test_serialize_deserialize() {
        let settings = Settings::default();
        let toml_str = toml::to_string(&settings).unwrap();
        let deserialized: Settings = toml::from_str(&toml_str).unwrap();

        assert_eq!(settings.dark_mode, deserialized.dark_mode);
        assert_eq!(settings.font_size, deserialized.font_size);
        assert_eq!(
            settings.performance.cache_size,
            deserialized.performance.cache_size
        );
        assert_eq!(
            settings.viewer.auto_expand_depth,
            deserialized.viewer.auto_expand_depth
        );
    }

    #[test]
    fn test_validation_valid_settings() {
        let settings = Settings::default();
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn test_validation_invalid_font_size() {
        let mut settings = Settings {
            font_size: 5.0,
            ..Default::default()
        };
        assert!(settings.validate().is_err());

        settings.font_size = 100.0; // Too large
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_validation_invalid_cache_size() {
        let mut settings = Settings::default();
        settings.performance.cache_size = 0;
        assert!(settings.validate().is_err());

        settings.performance.cache_size = 20000;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_validation_invalid_window_size() {
        let mut settings = Settings::default();
        settings.window.default_width = 100.0; // Too small
        assert!(settings.validate().is_err());

        settings.window.default_width = 10000.0; // Too large
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_validation_invalid_auto_expand_depth() {
        let mut settings = Settings::default();
        settings.viewer.auto_expand_depth = 15; // Too deep
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_migration() {
        let mut settings = Settings::default();
        settings.version = 0; // Simulate old version
        settings.migrate();
        assert_eq!(settings.version, Settings::CURRENT_VERSION);
    }

    #[test]
    fn test_performance_settings_defaults() {
        let perf = PerformanceSettings::default();
        assert_eq!(perf.cache_size, 100);
        assert_eq!(perf.max_file_size_mb, 500);
        assert_eq!(perf.max_recent_files, 10);
    }

    #[test]
    fn test_viewer_settings_defaults() {
        let viewer = ViewerSettings::default();
        assert_eq!(viewer.auto_expand_depth, 0);
        assert_eq!(viewer.scroll_margin, 3);
        assert!(viewer.syntax_highlighting);
        assert!(!viewer.show_line_numbers);
        assert_eq!(viewer.indent_size, 16.0);
    }

    #[test]
    fn test_ui_settings_defaults() {
        let ui = UiSettings::default();
        assert_eq!(ui.sidebar_width, 350.0);
        assert!(ui.remember_sidebar_state);
        assert!(ui.show_status_bar);
        assert!(ui.show_toolbar);
        assert!(ui.enable_animations);
    }
}
