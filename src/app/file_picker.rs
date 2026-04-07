use std::path::PathBuf;

use rfd::FileDialog;

use crate::{PLUGIN_MANAGER, plugin::Capability};

fn supported_files(plugins_enabled: bool) -> Vec<(String, Vec<String>)> {
    let mut all_supported_file_types = vec![(
        "JSON".to_string(),
        vec!["json".to_string(), "ndjson".to_string()],
    )];

    if plugins_enabled {
        if let Some(Some(plugin_manager)) = PLUGIN_MANAGER.get() {
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
    }

    all_supported_file_types
}

pub fn pick_file(plugins_enabled: bool) -> Option<PathBuf> {
    let mut fd = FileDialog::new();

    for (name, exts) in supported_files(plugins_enabled) {
        fd = fd.add_filter(name, &exts);
    }

    fd.pick_file()
}
