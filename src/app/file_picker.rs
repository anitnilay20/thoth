use std::path::PathBuf;

use rfd::FileDialog;

use crate::plugin::{Capability, runtime::active_manager};

/// Formats the engine reads natively, by the extensions they come with.
///
/// The extension only decides which filter a file appears under: an unnamed
/// format is still probed against DuckDB's readers when it is opened, so
/// picking a `.log` full of NDJSON gets a table either way.
const NATIVE: &[(&str, &[&str])] = &[
    ("JSON", &["json", "ndjson", "jsonl", "geojson"]),
    ("CSV", &["csv", "tsv"]),
    ("Parquet", &["parquet", "pq"]),
    ("Database", &["db", "duckdb", "sqlite", "sqlite3"]),
    ("Markdown", &["md", "markdown", "mdown", "mkd"]),
    ("Text", &["txt", "log", "text"]),
];

fn supported_files(plugins_enabled: bool) -> Vec<(String, Vec<String>)> {
    // "All supported" leads, because the common case is knowing the file you
    // want rather than its format. "All files" closes the list: anything that
    // is text at all opens, so a dialog that hides a file Thoth can read is
    // the dialog lying about what the app does.
    let mut all_supported_file_types: Vec<(String, Vec<String>)> = vec![(
        "All supported".to_string(),
        NATIVE
            .iter()
            .flat_map(|(_, exts)| exts.iter().map(|e| (*e).to_string()))
            .collect(),
    )];
    for (name, exts) in NATIVE {
        all_supported_file_types.push((
            (*name).to_string(),
            exts.iter().map(|e| (*e).to_string()).collect(),
        ));
    }

    if plugins_enabled && let Some(plugin_manager) = active_manager() {
        plugin_manager
            .get_all_plugin_by_capability(Capability::FileLoader)
            .iter()
            .for_each(|p| {
                p.file_loader.iter().for_each(|file_type| {
                    all_supported_file_types.push((
                        file_type.file_type.clone(),
                        file_type.supported_extensions.clone(),
                    ));
                });
            });
    }

    all_supported_file_types.push(("All files".to_string(), vec!["*".to_string()]));
    all_supported_file_types
}

pub fn pick_file(plugins_enabled: bool) -> Option<PathBuf> {
    let mut fd = FileDialog::new();

    for (name, exts) in supported_files(plugins_enabled) {
        fd = fd.add_filter(name, &exts);
    }

    fd.pick_file()
}
