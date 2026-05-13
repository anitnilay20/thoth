use std::{collections::HashMap, fs::File, io::BufReader};

use crate::{PLUGIN_MANAGER, plugin::Plugin, theme::Theme};

// TODO: Optimise theme plugin mod
pub fn all_theme_plugins<'a>() -> Vec<&'a Plugin> {
    if let Some(pm) = PLUGIN_MANAGER.get()
        && let Some(pm) = pm
    {
        return pm.get_all_plugin_by_capability(super::Capability::Theme);
    }

    vec![]
}

pub fn get_plugin_theme_catalog() -> Vec<(String, bool, String)> {
    all_theme_plugins()
        .iter()
        .flat_map(|plugin| {
            if let Some(theme_plugin) = &plugin.theme {
                let family = &theme_plugin.family;

                theme_plugin
                    .catalog
                    .iter()
                    .map(|c| (c.0.clone(), c.1, family.clone()))
                    .collect()
            } else {
                vec![]
            }
        })
        .collect()
}

pub fn get_plugin_theme_by_name(name: &str) -> Option<Theme> {
    let mut theme: Option<Theme> = None;

    all_theme_plugins().iter().for_each(|plugin| {
        if let Some(theme_plugin) = &plugin.theme
            && let Some(location) = &plugin.location
            && theme_plugin.catalog.iter().any(|(n, _)| n == name)
            && let Ok(file) = File::open(location).map_err(|err| {
                eprintln!(
                    "Error opening theme.json file for {} - {}",
                    plugin.name, err
                );
            })
        {
            {
                let rdr = BufReader::new(file);
                if let Ok(value) = serde_json::from_reader::<_, HashMap<String, Theme>>(rdr)
                    .map_err(|err| {
                        eprintln!("Error parsing theme file for {} - {}", plugin.name, err);
                    })
                {
                    theme = value.get(name).cloned();
                };
            };
        }
    });

    theme
}
