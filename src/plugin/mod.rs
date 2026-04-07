use std::fmt::Display;

use serde::{Deserialize, Serialize};

pub mod manager;
pub mod plugin_registry;
pub mod render_node;
pub mod wasm_file_viewer_loader;
pub mod wasm_loader;

// ── Capability-specific metadata ──────────────────────────────────────────────

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    FileLoader,
    FileViewer,
    DataSource,
    Exporter,
    SearchProvider,
    NewUIComponent,
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
    #[serde(rename = "file-loader")]
    pub file_loader: Vec<FileLoaderMeta>,
    #[serde(rename = "data-source")]
    pub data_source: Option<DataSourceMeta>,
    pub exporter: Option<ExporterMeta>,

    // ── Runtime-only fields (not in plugin.toml) ───────────────────────────────
    /// Path to icon.png next to plugin.wasm. Set by PluginManager at scan time.
    #[serde(skip)]
    pub icon_path: Option<std::path::PathBuf>,

    /// True when this plugin ships with the app and cannot be uninstalled.
    #[serde(skip)]
    pub bundled: bool,
}

pub trait PluginLifeCycle {
    fn on_load();
    fn on_close();
}

impl Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Capability::FileLoader => "File Loader".to_string(),
                Capability::FileViewer => "File Viewer".to_string(),
                Capability::DataSource => "Data Source".to_string(),
                Capability::Exporter => "Exporter".to_string(),
                Capability::SearchProvider => "Search Provider".to_string(),
                Capability::NewUIComponent => "New UI Component".to_string(),
            }
        )
    }
}
