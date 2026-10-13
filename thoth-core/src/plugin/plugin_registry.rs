use crate::plugin::{Capability, Plugin};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct PluginRegistry {
    capability_index: HashMap<Capability, HashSet<String>>,
    plugin_key: HashMap<String, Plugin>,
}

impl PluginRegistry {
    pub fn new() -> PluginRegistry {
        PluginRegistry {
            capability_index: HashMap::new(),
            plugin_key: HashMap::new(),
        }
    }

    pub fn get_all_plugins(&self) -> Vec<&Plugin> {
        let mut plugins: Vec<&Plugin> = self.plugin_key.values().collect();
        plugins.sort_by(|a, b| a.name.cmp(&b.name));
        plugins
    }

    pub fn add_plugin(&mut self, plugin: Plugin) {
        // If a plugin with this id already exists, remove its stale capability
        // entries before inserting the new version so no ghost memberships remain.
        if let Some(old) = self.plugin_key.remove(&plugin.id) {
            for cap in &old.capabilities {
                if let Some(set) = self.capability_index.get_mut(cap) {
                    set.remove(&old.id);
                    if set.is_empty() {
                        self.capability_index.remove(cap);
                    }
                }
            }
        }

        plugin.capabilities.iter().for_each(|c| {
            self.capability_index
                .entry(c.clone())
                .or_default()
                .insert(plugin.id.clone());
        });

        self.plugin_key.insert(plugin.id.clone(), plugin);
    }

    pub fn get_by_capability(&self, c: Capability) -> Vec<&Plugin> {
        if let Some(plugin_ids) = self.capability_index.get(&c) {
            plugin_ids.iter().flat_map(|f| self.get_by_id(f)).collect()
        } else {
            Vec::new()
        }
    }

    pub fn get_by_id(&self, id: &str) -> Option<&Plugin> {
        self.plugin_key.get(id)
    }

    pub fn remove_plugin(&mut self, id: &str) {
        if let Some(plugin) = self.plugin_key.remove(id) {
            for cap in &plugin.capabilities {
                if let Some(set) = self.capability_index.get_mut(cap) {
                    set.remove(id);
                    if set.is_empty() {
                        self.capability_index.remove(cap);
                    }
                }
            }
        }
    }

    pub fn get_installed_plugins(&self) -> HashMap<String, Plugin> {
        self.plugin_key.clone()
    }

    /// Find a FileLoader plugin that declares support for the given extension.
    /// `ext` should be lowercase without the leading dot (e.g. `"csv"`).
    /// Results are stable: IDs are sorted before iteration so the first
    /// matching plugin is always the same regardless of HashMap ordering.
    pub fn find_loader_for_extension(&self, ext: &str) -> Option<&Plugin> {
        let ext_lower = ext.to_lowercase();
        let mut ids: Vec<&String> = self
            .capability_index
            .get(&crate::plugin::Capability::FileLoader)?
            .iter()
            .collect();
        ids.sort();
        ids.into_iter()
            .flat_map(|id| self.plugin_key.get(id))
            .find(|p| {
                p.file_loader
                    .iter()
                    .any(|fl| fl.supported_extensions.iter().any(|e| e == &ext_lower))
            })
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
