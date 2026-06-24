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

    /// Parse a `[{"key":…,"value":…}]` payload, returning an error on malformed
    /// input so callers can distinguish "no settings" from "bad payload".
    pub fn try_from_json(json: &str) -> Result<Self, serde_json::Error> {
        let entries = serde_json::from_str(json)?;
        Ok(Self { entries })
    }

    /// Lenient wrapper over [`try_from_json`](Self::try_from_json): malformed
    /// input yields an empty map rather than an error, so a bad payload never
    /// crashes the plugin. Use [`try_from_json`](Self::try_from_json) when you
    /// need to detect parse failures.
    pub fn from_json(json: &str) -> Self {
        Self::try_from_json(json).unwrap_or_default()
    }

    /// The value for `key`, if present.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.key == key)
            .map(|e| e.value.as_str())
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
        self.entries
            .iter()
            .map(|e| (e.key.as_str(), e.value.as_str()))
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

#[cfg(test)]
mod tests {
    use super::SettingsMap;

    // ── from_json ─────────────────────────────────────────────────────────────

    #[test]
    fn from_json_parses_valid_payload() {
        let map = SettingsMap::from_json(r#"[{"key":"url","value":"https://x"}]"#);
        assert_eq!(map.get("url"), Some("https://x"));
    }

    #[test]
    fn from_json_empty_array_gives_empty_map() {
        let map = SettingsMap::from_json("[]");
        assert!(map.is_empty());
    }

    #[test]
    fn from_json_malformed_input_gives_empty_map() {
        let map = SettingsMap::from_json("not json at all");
        assert!(map.is_empty());
    }

    #[test]
    fn from_json_wrong_shape_gives_empty_map() {
        let map = SettingsMap::from_json(r#"{"key":"url","value":"x"}"#);
        assert!(map.is_empty());
    }

    #[test]
    fn try_from_json_parses_valid_payload() {
        let map = SettingsMap::try_from_json(r#"[{"key":"url","value":"https://x"}]"#).unwrap();
        assert_eq!(map.get("url"), Some("https://x"));
    }

    #[test]
    fn try_from_json_errors_on_malformed_input() {
        assert!(SettingsMap::try_from_json("not json at all").is_err());
    }

    #[test]
    fn try_from_json_errors_on_wrong_shape() {
        assert!(SettingsMap::try_from_json(r#"{"key":"url","value":"x"}"#).is_err());
    }

    #[test]
    fn from_json_preserves_multiple_entries() {
        let map = SettingsMap::from_json(r#"[{"key":"a","value":"1"},{"key":"b","value":"2"}]"#);
        assert_eq!(map.get("a"), Some("1"));
        assert_eq!(map.get("b"), Some("2"));
        assert_eq!(map.len(), 2);
    }

    // ── get / get_or ──────────────────────────────────────────────────────────

    #[test]
    fn get_returns_none_for_missing_key() {
        let map = SettingsMap::new();
        assert_eq!(map.get("missing"), None);
    }

    #[test]
    fn get_or_returns_default_for_missing_key() {
        let map = SettingsMap::new();
        assert_eq!(map.get_or("method", "GET"), "GET");
    }

    #[test]
    fn get_returns_first_value_for_duplicate_keys() {
        let map =
            SettingsMap::from_json(r#"[{"key":"x","value":"first"},{"key":"x","value":"second"}]"#);
        assert_eq!(map.get("x"), Some("first"));
    }

    // ── contains_key ──────────────────────────────────────────────────────────

    #[test]
    fn contains_key_true_when_present() {
        let map = SettingsMap::from_json(r#"[{"key":"k","value":"v"}]"#);
        assert!(map.contains_key("k"));
    }

    #[test]
    fn contains_key_false_when_absent() {
        let map = SettingsMap::new();
        assert!(!map.contains_key("missing"));
    }

    // ── insert ────────────────────────────────────────────────────────────────

    #[test]
    fn insert_adds_new_entry() {
        let mut map = SettingsMap::new();
        map.insert("key", "value");
        assert_eq!(map.get("key"), Some("value"));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn insert_replaces_existing_value_in_place() {
        let mut map = SettingsMap::from_json(r#"[{"key":"k","value":"old"}]"#);
        map.insert("k", "new");
        assert_eq!(map.get("k"), Some("new"));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn insert_preserves_order_of_remaining_entries() {
        let mut map =
            SettingsMap::from_json(r#"[{"key":"a","value":"1"},{"key":"b","value":"2"}]"#);
        map.insert("a", "updated");
        let keys: Vec<&str> = map.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, ["a", "b"]);
    }

    // ── with (builder) ────────────────────────────────────────────────────────

    #[test]
    fn with_chains_multiple_entries() {
        let map = SettingsMap::new()
            .with("url", "https://x")
            .with("method", "POST");
        assert_eq!(map.get("url"), Some("https://x"));
        assert_eq!(map.get("method"), Some("POST"));
        assert_eq!(map.len(), 2);
    }

    // ── to_json ───────────────────────────────────────────────────────────────

    #[test]
    fn to_json_empty_map_produces_empty_array() {
        let json = SettingsMap::new().to_json();
        assert_eq!(json, "[]");
    }

    #[test]
    fn to_json_round_trips_single_entry() {
        let json = SettingsMap::new().with("url", "https://x").to_json();
        assert_eq!(json, r#"[{"key":"url","value":"https://x"}]"#);
    }

    #[test]
    fn to_json_round_trips_multiple_entries_in_order() {
        let json = SettingsMap::new()
            .with("url", "https://x")
            .with("method", "POST")
            .to_json();
        assert_eq!(
            json,
            r#"[{"key":"url","value":"https://x"},{"key":"method","value":"POST"}]"#
        );
    }

    #[test]
    fn from_json_then_to_json_is_identity() {
        let original = r#"[{"key":"a","value":"1"},{"key":"b","value":"2"}]"#;
        let json = SettingsMap::from_json(original).to_json();
        assert_eq!(json, original);
    }

    // ── iter / len / is_empty ─────────────────────────────────────────────────

    #[test]
    fn iter_yields_entries_in_insertion_order() {
        let map = SettingsMap::new().with("z", "1").with("a", "2");
        let pairs: Vec<(&str, &str)> = map.iter().collect();
        assert_eq!(pairs, [("z", "1"), ("a", "2")]);
    }

    #[test]
    fn is_empty_true_on_new_map() {
        assert!(SettingsMap::new().is_empty());
    }

    #[test]
    fn is_empty_false_after_insert() {
        let map = SettingsMap::new().with("k", "v");
        assert!(!map.is_empty());
    }

    // ── from_iter ─────────────────────────────────────────────────────────────

    #[test]
    fn from_iter_builds_map_from_string_pairs() {
        let pairs = vec![
            ("url".to_string(), "https://example.com".to_string()),
            ("method".to_string(), "GET".to_string()),
        ];
        let map: SettingsMap = pairs.into_iter().collect();
        assert_eq!(map.get("url"), Some("https://example.com"));
        assert_eq!(map.get("method"), Some("GET"));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn from_iter_empty_iterator_gives_empty_map() {
        let map: SettingsMap = std::iter::empty::<(String, String)>().collect();
        assert!(map.is_empty());
    }
}
