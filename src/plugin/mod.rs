use std::fmt::Display;

use crate::helpers::default_rate_limit;
use serde::{Deserialize, Serialize};

pub mod manager;
pub mod marketplace;
pub mod network_policy;
pub mod plugin_registry;
pub mod plugin_ui_host;
pub mod render_node;
pub mod signals;
pub mod theme_plugin;
pub mod wasm_data_source;
pub mod wasm_file_viewer_loader;
pub mod wasm_loader;
pub mod wasm_plugin_settings;
pub mod wasm_ui_component;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkDeclarations {
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default)]
    pub require_https: bool,
    #[serde(default = "default_rate_limit")]
    pub rate_limit_rpm: u32,
}

/// Metadata required when a plugin declares the `file-loader` capability.
/// Deserialised from the `[file-loader]` section of `plugin.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileLoaderMeta {
    /// File extensions this plugin handles, e.g. `["csv", "tsv"]`.
    /// Must be lowercase, without the leading dot.
    #[serde(rename = "file-type")]
    pub file_type: String,

    #[serde(rename = "supported-extensions")]
    pub supported_extensions: Vec<String>,
}

/// Metadata required when a plugin declares the `data-source` capability.
/// Deserialised from the `[data-source]` section of `plugin.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceMeta {
    /// Human-readable connection type shown in the "Connect" dialog,
    /// e.g. `"PostgreSQL"`, `"REST API"`.
    pub display_name: String,
}

/// Metadata required when a plugin declares the `exporter` capability.
/// Deserialised from the `[exporter]` section of `plugin.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExporterMeta {
    /// Output file extension without dot, e.g. `"csv"`.
    pub output_extension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeMeta {
    pub family: String,

    // Catalog of themes featured by the plugin.
    // List of Display name and if its a dark mode theme.
    pub catalog: Vec<(String, bool)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    FileLoader,
    FileViewer,
    DataSource,
    Exporter,
    SearchProvider,
    #[serde(rename = "new-ui-component")]
    NewUIComponent,
    Theme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<Capability>,
    pub location: Option<String>,
    pub author: String,
    pub homepage: Option<String>,
    // ── Capability-specific metadata (from plugin.toml sections) ──────────────
    #[serde(rename = "file-loader", default)]
    pub file_loader: Vec<FileLoaderMeta>,

    #[serde(rename = "data-source")]
    pub data_source: Option<DataSourceMeta>,
    pub exporter: Option<ExporterMeta>,

    #[serde(default)]
    pub network: Option<NetworkDeclarations>,

    #[serde(rename = "theme")]
    pub theme: Option<ThemeMeta>,

    /// Phosphor glyph character for the sidebar icon button.
    /// Set this in plugin.toml, e.g. `icon = "\u{E28C}"`.
    /// Falls back to the generic database icon when absent.
    #[serde(default)]
    pub icon: Option<String>,

    // ── Runtime-only fields (not in plugin.toml) ───────────────────────────────
    /// Path to icon.png next to plugin.wasm. Set by PluginManager at scan time.
    #[serde(skip)]
    pub icon_path: Option<std::path::PathBuf>,

    /// True when this plugin ships with the app and cannot be uninstalled.
    #[serde(skip)]
    pub bundled: bool,
}

pub trait PluginLifeCycle {
    fn on_load(&mut self);
    fn on_close(&mut self);
}

impl Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Capability::FileLoader => "File Loader",
                Capability::FileViewer => "File Viewer",
                Capability::DataSource => "Data Source",
                Capability::Exporter => "Exporter",
                Capability::SearchProvider => "Search Provider",
                Capability::NewUIComponent => "New UI Component",
                Capability::Theme => "Theme",
            }
        )
    }
}
