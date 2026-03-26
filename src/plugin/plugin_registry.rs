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
            plugin_ids
                .iter()
                .flat_map(|f| self.get_by_id(f))
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn get_by_id(&self, id: &str) -> Option<&Plugin> {
        self.plugin_key.get(id)
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
