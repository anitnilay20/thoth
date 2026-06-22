//! Parse and build the plugin settings payload.
//!
//! The host passes settings to `on-load` / `on-setting-change` as a JSON array
//! of `{ "key": ..., "value": ... }` records (the WIT `setting-data` shape),
//! e.g. `[{"key":"url","value":"https://…"},{"key":"method","value":"GET"}]`.
//!
//! [`SettingsMap`] reads that payload ergonomically and builds it back:
//!
//! ```
//! use thoth_plugin_sdk::settings::SettingsMap;
//!
//! // Parse an incoming payload.
//! let map = SettingsMap::from_json(r#"[{"key":"url","value":"https://x"}]"#);
//! assert_eq!(map.get("url"), Some("https://x"));
//! assert_eq!(map.get_or("method", "GET"), "GET");
//!
//! // Build one to hand back.
//! let json = SettingsMap::new()
//!     .with("url", "https://x")
//!     .with("method", "POST")
//!     .to_json();
//! assert_eq!(json, r#"[{"key":"url","value":"https://x"},{"key":"method","value":"POST"}]"#);
//! ```

use serde::{Deserialize, Serialize};

/// One `key`/`value` settings entry — the WIT `setting-data` record.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SettingEntry {
    /// Setting key.
    pub key: String,
    /// Setting value (always a string; parse as needed).
    pub value: String,
}

/// An ordered map over the plugin settings payload.
///
/// Order is preserved (it round-trips through [`to_json`](Self::to_json)).
/// Reads return the value for the first matching key.
#[derive(Clone, Debug, Default)]
pub struct SettingsMap {
    entries: Vec<SettingEntry>,
}

impl SettingsMap {
    /// Create an empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a `[{"key":…,"value":…}]` payload. Malformed input yields an empty
    /// map rather than an error, so a bad payload never crashes the plugin.
    pub fn from_json(json: &str) -> Self {
        let entries = serde_json::from_str(json).unwrap_or_default();
        Self { entries }
    }

    /// The value for `key`, if present.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.iter().find(|e| e.key == key).map(|e| e.value.as_str())
    }

    /// The value for `key`, or `default` when absent.
    pub fn get_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.get(key).unwrap_or(default)
    }

    /// Whether `key` is present.
    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|e| e.key == key)
    }

    /// Insert or replace a value in place.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        if let Some(e) = self.entries.iter_mut().find(|e| e.key == key) {
            e.value = value;
        } else {
            self.entries.push(SettingEntry { key, value });
        }
    }

    /// Builder-style [`insert`](Self::insert) that returns `self` for chaining.
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.insert(key, value);
        self
    }

    /// Iterate over the `(key, value)` entries in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|e| (e.key.as_str(), e.value.as_str()))
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Serialize back to the `[{"key":…,"value":…}]` payload format.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.entries).unwrap_or_else(|_| "[]".to_string())
    }
}

impl FromIterator<(String, String)> for SettingsMap {
    fn from_iter<I: IntoIterator<Item = (String, String)>>(iter: I) -> Self {
        Self {
            entries: iter
                .into_iter()
                .map(|(key, value)| SettingEntry { key, value })
                .collect(),
        }
    }
}
