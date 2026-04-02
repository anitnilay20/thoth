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

    pub fn add_plugin(&mut self, plugin: Plugin) {
        plugin.capabilities.iter().for_each(|c| {
            let capability = self.capability_index.entry(c.clone()).or_default();
            capability.insert(plugin.id.clone());
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

    /// Find a FileLoader plugin that declares support for the given extension.
    /// `ext` should be lowercase without the leading dot (e.g. `"csv"`).
    pub fn find_loader_for_extension(&self, ext: &str) -> Option<&Plugin> {
        let ext_lower = ext.to_lowercase();
        self.capability_index
            .get(&crate::plugin::Capability::FileLoader)?
            .iter()
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
