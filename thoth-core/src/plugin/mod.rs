use std::fmt::Display;

use crate::helpers::default_rate_limit;
use serde::{Deserialize, Serialize};

pub mod dataset_grants;
pub mod manager;
pub mod marketplace;
pub mod network_policy;
pub mod plugin_registry;
pub mod plugin_ui_host;
pub mod render_node;
pub mod runtime;
pub mod signals;
pub mod theme_plugin;
pub mod wasm_cli;
pub mod wasm_data_source;
pub mod wasm_exporter;
pub mod wasm_file_viewer_loader;
pub mod wasm_loader;
pub mod wasm_plugin_settings;
pub mod wasm_renderer;
pub mod wasm_ui_component;
pub mod websocket;

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

/// Metadata required when a plugin declares the `duckdb-extension` capability.
/// Deserialised from the `[duckdb-extension]` section of `plugin.toml`.
///
/// The plugin carries no binary of its own: DuckDB already hosts every
/// extension for every platform it supports, keyed by its own version, and
/// picks the right build itself. Shipping the `.duckdb_extension` files here
/// instead would mean owning that whole matrix by hand and re-uploading it on
/// every DuckDB release — so the plugin *declares* the reader and the engine
/// fetches it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DuckdbExtensionMeta {
    /// DuckDB's name for the extension, as `INSTALL` takes it.
    pub extension: String,
    /// `"core"` (the default) or `"community"` — the latter needs
    /// `INSTALL … FROM community`, and an install that omits it fails with a
    /// download error that reads like the network is down.
    #[serde(default)]
    pub repository: Option<String>,
    /// File extensions this reader opens, without dots.
    #[serde(default)]
    pub supported_extensions: Vec<String>,
    /// What it lets the user open, in their words rather than DuckDB's.
    #[serde(default)]
    pub unlocks: Option<String>,
    /// Roughly what fetching it costs, so the offer can be honest about it.
    #[serde(default)]
    pub download_size: Option<String>,
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
    /// Opts the plugin in as a dataset producer for the data bus (#113).
    DataProducer,
    /// Opts the plugin in as a dataset renderer (#135): it presents a host-owned
    /// dataset as an extra view format in a DataView.
    Renderer,
    /// Exposes display-free commands through the `plugin-cli` WIT interface.
    Cli,
    /// Declares one of DuckDB's optional file readers. The plugin is metadata
    /// only — installing it tells the engine to fetch the extension.
    DuckdbExtension,
}

#[cfg(test)]
mod duckdb_reader_tests {
    use super::*;

    /// The reader plugins as they are actually published, parsed by the code
    /// that will parse them in the field.
    #[test]
    fn a_published_reader_plugin_parses() {
        let toml = r#"
id          = "com.thoth.duckdb-excel"
name        = "Excel Reader"
version     = "1.0.0"
description = "Open Excel workbooks as tables you can query."
author      = "Thoth contributors"
icon        = "\uEB6C"

capabilities = ["duckdb-extension"]

[duckdb-extension]
extension            = "excel"
repository           = "core"
supported-extensions = ["xlsx", "xlsm"]
unlocks              = "Excel workbooks (.xlsx, .xlsm)"
download-size        = "7.5 MB"
"#;
        let plugin: Plugin = toml::from_str(toml).expect("a published plugin.toml must parse");
        assert_eq!(plugin.capabilities, vec![Capability::DuckdbExtension]);

        let meta = plugin.duckdb_extension.expect("the capability's section");
        assert_eq!(meta.extension, "excel");
        assert_eq!(meta.repository.as_deref(), Some("core"));
        assert_eq!(meta.supported_extensions, ["xlsx", "xlsm"]);
        assert_eq!(meta.download_size.as_deref(), Some("7.5 MB"));
    }

    #[test]
    fn a_community_reader_keeps_its_repository() {
        // Losing this turns `INSTALL arrow FROM community` into `INSTALL
        // arrow`, which fails with a download error that reads like the
        // network is down.
        let toml = r#"
id          = "com.thoth.duckdb-arrow"
name        = "Arrow Reader"
version     = "1.0.0"
description = "Open Arrow IPC streams."
author      = "Thoth contributors"
capabilities = ["duckdb-extension"]

[duckdb-extension]
extension            = "arrow"
repository           = "community"
supported-extensions = ["arrow", "arrows", "ipc"]
"#;
        let plugin: Plugin = toml::from_str(toml).unwrap();
        let meta = plugin.duckdb_extension.unwrap();
        assert_eq!(meta.repository.as_deref(), Some("community"));
        // The optional prose is genuinely optional.
        assert!(meta.unlocks.is_none());
        assert!(meta.download_size.is_none());
    }

    #[test]
    fn a_metadata_only_plugin_is_not_asked_for_wasm() {
        // Reader plugins ship no `plugin.wasm` — there is nothing to run. The
        // scanner used to key off `theme.json` existing, so every one of them
        // was skipped with "failed to read from .../plugin.wasm".
        assert!(!Capability::DuckdbExtension.needs_runtime());
        assert!(!Capability::Theme.needs_runtime());

        // Everything else does have code behind it.
        for capability in [
            Capability::FileLoader,
            Capability::FileViewer,
            Capability::DataSource,
            Capability::Exporter,
            Capability::SearchProvider,
            Capability::NewUIComponent,
            Capability::DataProducer,
            Capability::Renderer,
            Capability::Cli,
        ] {
            assert!(
                capability.needs_runtime(),
                "{capability} has no wasm to load"
            );
        }
    }

    #[test]
    fn the_capability_name_is_the_one_the_manifest_writes() {
        // serde kebab-cases it; a plugin.toml saying anything else would be
        // silently ignored rather than rejected.
        assert_eq!(
            serde_json::to_string(&Capability::DuckdbExtension).unwrap(),
            "\"duckdb-extension\""
        );
    }
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

    #[serde(rename = "duckdb-extension")]
    pub duckdb_extension: Option<DuckdbExtensionMeta>,

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

impl Capability {
    /// Whether this capability is carried out by running WASM.
    ///
    /// Most are. Two are pure metadata: a theme is a colour table, and a
    /// DuckDB reader is a declaration that the engine should fetch an
    /// extension. Neither has code to run, so neither ships a `plugin.wasm` —
    /// and a scanner that insists on one skips them entirely.
    pub fn needs_runtime(&self) -> bool {
        !matches!(self, Capability::Theme | Capability::DuckdbExtension)
    }
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
                Capability::DataProducer => "Data Producer",
                Capability::Renderer => "Renderer",
                Capability::Cli => "CLI",
                Capability::DuckdbExtension => "File Reader",
            }
        )
    }
}
