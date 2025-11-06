use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Dark mode enabled
    pub dark_mode: bool,

    /// Font size for UI
    pub font_size: f32,

    /// Window settings (for future multi-window support)
    pub window: WindowSettings,

    /// Auto-update settings
    pub updates: UpdateSettings,
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

impl Default for Settings {
    fn default() -> Self {
        Self {
            dark_mode: true,
            font_size: 14.0,
            window: WindowSettings::default(),
            updates: UpdateSettings::default(),
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

impl Settings {
    /// Get the path to the settings file
    /// Returns: ~/.config/thoth/settings.toml on Linux/macOS
    ///          %APPDATA%/thoth/settings.toml on Windows
    pub fn settings_file_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Failed to get config directory")?;

        let thoth_config_dir = config_dir.join("thoth");

        // Create directory if it doesn't exist
        if !thoth_config_dir.exists() {
            std::fs::create_dir_all(&thoth_config_dir)
                .context("Failed to create thoth config directory")?;
        }

        Ok(thoth_config_dir.join("settings.toml"))
    }

    /// Load settings from file, or create default if file doesn't exist
    pub fn load() -> Result<Self> {
        let settings_path = Self::settings_file_path()?;

        if settings_path.exists() {
            let contents =
                std::fs::read_to_string(&settings_path).context("Failed to read settings file")?;

            let settings: Settings =
                toml::from_str(&contents).context("Failed to parse settings file")?;

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

        let toml_string = toml::to_string_pretty(self).context("Failed to serialize settings")?;

        std::fs::write(&settings_path, toml_string).context("Failed to write settings file")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.dark_mode, true);
        assert_eq!(settings.font_size, 14.0);
        assert_eq!(settings.window.default_width, 1200.0);
        assert_eq!(settings.window.default_height, 800.0);
    }

    #[test]
    fn test_serialize_deserialize() {
        let settings = Settings::default();
        let toml_str = toml::to_string(&settings).unwrap();
        let deserialized: Settings = toml::from_str(&toml_str).unwrap();

        assert_eq!(settings.dark_mode, deserialized.dark_mode);
        assert_eq!(settings.font_size, deserialized.font_size);
    }
}
