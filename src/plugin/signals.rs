//! Host-side registry for the plugin **signals** channel.
//!
//! Plugins push tiny status/metric key-values via the `signals` WIT import
//! (see [`wasm_data_source`]); the host aggregates them here and the status bar
//! renders them ([`crate::components::status_bar`]). This is the push half of
//! the plugin data ecosystem (#110); the pull half — datasets — is a separate
//! channel (#113/#114).
//!
//! Semantics (per issue #110):
//! - **Last-write-wins** per `(plugin_id, key)`.
//! - **TTL-expiring**: a signal with `ttl_ms > 0` disappears that long after it
//!   was received; `ttl_ms == 0` is sticky until the plugin overwrites it.
//! - **Source-attributed**: signals are grouped by the emitting plugin.
//!
//! The registry is a process-global behind a `Mutex`, mirroring the
//! [`ConsentManager`](crate::consent::manager::ConsentManager) pattern: the
//! plugin host writes to it during a WASM call (possibly on a worker thread) and
//! the UI thread reads a snapshot each frame.
//!
//! [`wasm_data_source`]: crate::plugin::wasm_data_source

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Health of a plugin's current activity, mirroring the `signals.status` WIT
/// enum. Host-owned so the registry doesn't depend on any world's bindgen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignalStatus {
    Ready,
    Loading,
    Error,
}

/// One live signal value.
#[derive(Clone, Debug)]
pub struct Signal {
    pub key: String,
    pub value: String,
    pub status: SignalStatus,
    /// When this signal expires; `None` = sticky.
    expires_at: Option<Instant>,
}

impl Signal {
    fn is_expired(&self, now: Instant) -> bool {
        self.expires_at.is_some_and(|deadline| now >= deadline)
    }
}

/// A plugin's live signals, for display (grouped by source).
#[derive(Clone, Debug)]
pub struct PluginSignals {
    pub plugin_id: String,
    pub signals: Vec<Signal>,
}

#[derive(Default)]
struct Registry {
    /// `(plugin_id, key)` → signal. Last-write-wins.
    map: HashMap<(String, String), Signal>,
    /// Stable insertion order of keys so the status bar doesn't reshuffle
    /// every frame (HashMap iteration order is unspecified).
    order: Vec<(String, String)>,
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(Registry::default()));

// Bounds so a buggy or hostile (sandboxed) plugin can't grow the registry
// without limit: at most this many distinct keys per plugin, and each key /
// value truncated to this many bytes before storage. A `ttl_ms == 0` signal is
// sticky, but its retained lifetime is still bounded — `retain_plugins` drops
// it when the plugin's pane closes.
const MAX_KEYS_PER_PLUGIN: usize = 16;
const MAX_KEY_LEN: usize = 64;
const MAX_VALUE_LEN: usize = 256;

/// Truncate `s` to at most `max` bytes, landing on a char boundary.
fn truncate_bytes(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// Record a signal pushed by `plugin_id`. `ttl_ms == 0` is sticky. Over-long
/// keys/values are truncated, and a new key beyond the per-plugin cap is
/// dropped; last-write-wins on existing keys is unaffected.
pub fn emit(plugin_id: &str, key: String, value: String, status: SignalStatus, ttl_ms: u32) {
    let key = truncate_bytes(&key, MAX_KEY_LEN);
    let value = truncate_bytes(&value, MAX_VALUE_LEN);
    let expires_at = (ttl_ms > 0).then(|| Instant::now() + Duration::from_millis(ttl_ms as u64));
    if let Ok(mut reg) = REGISTRY.lock() {
        let id = (plugin_id.to_string(), key.clone());
        if !reg.map.contains_key(&id) {
            // Cap distinct keys per plugin; drop new keys past the limit.
            let keys_for_plugin = reg.map.keys().filter(|(pid, _)| pid == plugin_id).count();
            if keys_for_plugin >= MAX_KEYS_PER_PLUGIN {
                return;
            }
            reg.order.push(id.clone());
        }
        reg.map.insert(
            id,
            Signal {
                key,
                value,
                status,
                expires_at,
            },
        );
    }
}

/// Live (non-expired) signals grouped by plugin, in stable insertion order.
/// Prunes expired entries as a side effect so the map doesn't grow unbounded.
pub fn snapshot() -> Vec<PluginSignals> {
    let now = Instant::now();
    let Ok(mut reg) = REGISTRY.lock() else {
        return Vec::new();
    };

    // Drop expired entries from both the map and the order list.
    let expired: Vec<(String, String)> = reg
        .map
        .iter()
        .filter(|(_, s)| s.is_expired(now))
        .map(|(id, _)| id.clone())
        .collect();
    for id in expired {
        reg.map.remove(&id);
        reg.order.retain(|o| o != &id);
    }

    // Group by plugin, preserving first-seen order for both plugins and keys.
    let mut groups: Vec<PluginSignals> = Vec::new();
    for id in &reg.order {
        let Some(sig) = reg.map.get(id) else { continue };
        let (plugin_id, _) = id;
        match groups.iter_mut().find(|g| &g.plugin_id == plugin_id) {
            Some(g) => g.signals.push(sig.clone()),
            None => groups.push(PluginSignals {
                plugin_id: plugin_id.clone(),
                signals: vec![sig.clone()],
            }),
        }
    }
    groups
}

/// Drop signals for every plugin **not** in `open`. Called each frame with the
/// set of plugin ids that still have a live pane, so a closed pane's signals
/// stop showing in the status bar. Windows are separate OS processes, so this
/// process's `open` set is authoritative for its own registry.
pub fn retain_plugins(open: &std::collections::HashSet<String>) {
    if let Ok(mut reg) = REGISTRY.lock() {
        reg.map.retain(|(pid, _), _| open.contains(pid));
        reg.order.retain(|(pid, _)| open.contains(pid));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The registry is a process-global; tests run in parallel, so serialize
    // them and clear state up front under the same lock.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if let Ok(mut reg) = REGISTRY.lock() {
            reg.map.clear();
            reg.order.clear();
        }
        guard
    }

    #[test]
    fn last_write_wins_per_key() {
        let _guard = reset();
        emit("p", "rows".into(), "1".into(), SignalStatus::Loading, 0);
        emit("p", "rows".into(), "42".into(), SignalStatus::Ready, 0);
        let snap = snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].signals.len(), 1);
        assert_eq!(snap[0].signals[0].value, "42");
        assert_eq!(snap[0].signals[0].status, SignalStatus::Ready);
    }

    #[test]
    fn groups_by_plugin_in_stable_order() {
        let _guard = reset();
        emit("a", "x".into(), "1".into(), SignalStatus::Ready, 0);
        emit("b", "y".into(), "2".into(), SignalStatus::Ready, 0);
        emit("a", "z".into(), "3".into(), SignalStatus::Ready, 0);
        let snap = snapshot();
        assert_eq!(snap.len(), 2);
        assert_eq!(snap[0].plugin_id, "a");
        assert_eq!(snap[0].signals.len(), 2);
        assert_eq!(snap[1].plugin_id, "b");
    }

    #[test]
    fn retain_plugins_drops_closed() {
        let _guard = reset();
        emit("a", "x".into(), "1".into(), SignalStatus::Ready, 0);
        emit("b", "y".into(), "2".into(), SignalStatus::Ready, 0);
        let open = std::collections::HashSet::from(["b".to_string()]);
        retain_plugins(&open);
        let snap = snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].plugin_id, "b");
    }

    #[test]
    fn caps_keys_per_plugin_and_truncates() {
        let _guard = reset();
        // One extra key beyond the cap is dropped.
        for i in 0..(MAX_KEYS_PER_PLUGIN + 5) {
            emit("p", format!("k{i}"), "v".into(), SignalStatus::Ready, 0);
        }
        let snap = snapshot();
        assert_eq!(snap[0].signals.len(), MAX_KEYS_PER_PLUGIN);
        // Over-long value is truncated to the byte cap.
        emit("q", "big".into(), "x".repeat(1000), SignalStatus::Ready, 0);
        let snap = snapshot();
        let q = snap.iter().find(|g| g.plugin_id == "q").unwrap();
        assert_eq!(q.signals[0].value.len(), MAX_VALUE_LEN);
    }
}
