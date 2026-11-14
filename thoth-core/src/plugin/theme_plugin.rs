use std::{collections::HashMap, fs::File, io::BufReader};

use crate::theme::Theme;

pub fn get_plugin_theme_catalog() -> Vec<(String, bool, String)> {
    super::runtime::active_manager()
        .map(|manager| {
            manager
                .get_all_plugin_by_capability(super::Capability::Theme)
                .into_iter()
                .filter_map(|plugin| plugin.theme.as_ref())
                .flat_map(|theme| {
                    theme
                        .catalog
                        .iter()
                        .map(|entry| (entry.0.clone(), entry.1, theme.family.clone()))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn get_plugin_theme_by_name(name: &str) -> Option<Theme> {
    let manager = super::runtime::active_manager()?;
    let plugin = manager
        .get_all_plugin_by_capability(super::Capability::Theme)
        .into_iter()
        .find(|plugin| {
            plugin
                .theme
                .as_ref()
                .is_some_and(|theme| theme.catalog.iter().any(|(entry, _)| entry == name))
        })?;
    let location = plugin.location.as_ref()?;
    let file = File::open(location)
        .map_err(|error| {
            eprintln!(
                "Error opening theme.json file for {} - {}",
                plugin.name, error
            );
        })
        .ok()?;
    let themes = serde_json::from_reader::<_, HashMap<String, Theme>>(BufReader::new(file))
        .map_err(|error| {
            eprintln!("Error parsing theme file for {} - {}", plugin.name, error);
        })
        .ok()?;
    themes.get(name).cloned()
}
