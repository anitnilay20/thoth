use std::path::PathBuf;

use rfd::FileDialog;

use crate::{PLUGIN_MANAGER, plugin::Capability};

fn supported_files() -> Vec<(String, Vec<String>)> {
    let mut all_suported_file_types = vec![(
        "JSON".to_string(),
        vec!["json".to_string(), "ndjson".to_string()],
    )];

    if let Some(Some(plugin_manager)) = PLUGIN_MANAGER.get() {
        plugin_manager
            .get_all_plugin_by_capability(Capability::FileLoader)
            .iter()
            .for_each(|p| {
                p.file_loader.iter().for_each(|file_type| {
                    all_suported_file_types.push((
                        file_type.file_type.clone(),
                        file_type.supported_extensions.clone(),
                    ));
                });
            });
    }

    all_suported_file_types
}

pub fn pick_file() -> Option<PathBuf> {
    let mut fd = FileDialog::new().add_filter("JSON", &["json", "ndjson"]);

    supported_files().iter().for_each(|f| {
        fd = fd.clone().add_filter(f.0.clone(), &f.1);
    });

    fd.pick_file()
}
