//! Persisted and session-enforced host capability grants for plugins (#119).
//!
//! A plugin declares the host capabilities it requires in `plugin.toml`:
//! ```toml
//! permissions = ["network", "tcp", "filesystem", "secure-storage", "tabs", "dataset-read"]
//! ```
//! On first load (when `permissions` is non-empty and not yet recorded in
//! `Settings::plugins.capability_grants`), the host raises a single
//! capability-permissions consent prompt. Approving persists the grant; denying
//! records an empty grant for the session so the plugin stays blocked without
//! re-prompting on every call.
//!
//! At the WASM host boundary, every gated import (`http-client`, `websocket`,
//! `tcp-client`, `file-dialog`, `secure-storage`, `ui-tabs`, `dataset-read`)
//! checks [`check`] before executing.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

use super::HostPermission;

/// Active grants per plugin ID (`plugin_id -> granted permissions`).
/// A plugin ID absent from this map has not been granted any permissions.
static GRANTS: LazyLock<Mutex<HashMap<String, HashSet<HostPermission>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Plugin IDs that already have a capability prompt in flight or resolved
/// this session, preventing duplicate modals across frames.
static PROMPTED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Seed the in-memory grant table from persisted `Settings::plugins.capability_grants`.
pub fn seed_from_settings(saved: &HashMap<String, Vec<HostPermission>>) {
    if let Ok(mut g) = GRANTS.lock() {
        for (plugin_id, perms) in saved {
            g.insert(plugin_id.clone(), perms.iter().copied().collect());
        }
    }
}

/// Record an approved set of permissions for `plugin_id`.
pub fn grant(plugin_id: &str, perms: &[HostPermission]) {
    if let Ok(mut g) = GRANTS.lock() {
        g.insert(plugin_id.to_string(), perms.iter().copied().collect());
    }
    if let Ok(mut p) = PROMPTED.lock() {
        p.insert(plugin_id.to_string());
    }
}

/// Record a denial for `plugin_id` (empty grant set for this session).
pub fn deny(plugin_id: &str) {
    if let Ok(mut g) = GRANTS.lock() {
        g.insert(plugin_id.to_string(), HashSet::new());
    }
    if let Ok(mut p) = PROMPTED.lock() {
        p.insert(plugin_id.to_string());
    }
}

/// Mark `plugin_id` as having a prompt in flight. Returns `true` if this call
/// is the first to claim the prompt (and `false` if already prompted).
pub fn mark_prompted(plugin_id: &str) -> bool {
    PROMPTED
        .lock()
        .map(|mut p| p.insert(plugin_id.to_string()))
        .unwrap_or(false)
}

/// Whether `plugin_id` currently holds `perm`.
pub fn is_allowed(plugin_id: &str, perm: HostPermission) -> bool {
    GRANTS
        .lock()
        .map(|g| g.get(plugin_id).is_some_and(|s| s.contains(&perm)))
        .unwrap_or(false)
}

/// Enforce `perm` for `plugin_id` at a WASM host boundary.
pub fn check(plugin_id: &str, perm: HostPermission) -> Result<(), String> {
    if is_allowed(plugin_id, perm) {
        Ok(())
    } else {
        Err(format!(
            "Plugin '{plugin_id}' does not have permission '{perm}' granted"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_deny_and_check_lifecycle() {
        let id = "test.cap.lifecycle";
        assert!(!is_allowed(id, HostPermission::Network));
        assert!(check(id, HostPermission::Network).is_err());

        grant(id, &[HostPermission::Network, HostPermission::Tabs]);
        assert!(is_allowed(id, HostPermission::Network));
        assert!(is_allowed(id, HostPermission::Tabs));
        assert!(!is_allowed(id, HostPermission::Tcp));
        assert!(check(id, HostPermission::Tcp).is_err());

        deny(id);
        assert!(!is_allowed(id, HostPermission::Network));
        assert!(!is_allowed(id, HostPermission::Tabs));
    }

    #[test]
    fn seed_from_settings_populates_grants() {
        let id = "test.cap.seed";
        let mut saved = HashMap::new();
        saved.insert(
            id.to_string(),
            vec![HostPermission::DatasetRead, HostPermission::SecureStorage],
        );
        seed_from_settings(&saved);
        assert!(is_allowed(id, HostPermission::DatasetRead));
        assert!(is_allowed(id, HostPermission::SecureStorage));
        assert!(!is_allowed(id, HostPermission::Filesystem));
    }

    #[test]
    fn mark_prompted_deduplicates() {
        let id = "test.cap.prompted";
        assert!(mark_prompted(id));
        assert!(!mark_prompted(id));
    }
}
