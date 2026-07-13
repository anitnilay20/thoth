//! Host-side registry for the plugin **signals** channel.
//!
//! Plugins push tiny status/metric key-values via the `signals` WIT import
//! (see [`wasm_data_source`]); the host aggregates them here and the status bar
//! renders them ([`crate::components::status_bar`]). This is the push half of
//! the plugin data ecosystem (#110); the pull half — datasets — is a separate
//! channel (#113/#114).
//!
//! Semantics (per issue #110, extended for #111):
//! - **Last-write-wins** per `(instance_id, key)` — keyed by plugin *instance*
//!   (pane), so two tabs of the same plugin keep independent status.
//! - **TTL-expiring**: a signal with `ttl_ms > 0` disappears that long after it
//!   was received; `ttl_ms == 0` is sticky until the plugin overwrites it.
//! - **Source-attributed**: each signal carries its `plugin_id` for display;
//!   signals are grouped by instance.
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
    /// Source plugin id, for the display label (the registry keys on instance).
    pub plugin_id: String,
    /// When this signal expires; `None` = sticky.
    expires_at: Option<Instant>,
}

impl Signal {
    fn is_expired(&self, now: Instant) -> bool {
        self.expires_at.is_some_and(|deadline| now >= deadline)
    }
}

/// One plugin instance's live signals, for display (grouped by instance so two
/// tabs of the same plugin stay independent).
#[derive(Clone, Debug)]
pub struct PluginSignals {
    pub instance_id: String,
    pub plugin_id: String,
    pub signals: Vec<Signal>,
}

#[derive(Default)]
struct Registry {
    /// `(instance_id, key)` → signal. Last-write-wins per instance+key.
    map: HashMap<(String, String), Signal>,
    /// Stable insertion order of keys so the status bar doesn't reshuffle
    /// every frame (HashMap iteration order is unspecified).
    order: Vec<(String, String)>,
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(Registry::default()));

// Bounds so a buggy or hostile (sandboxed) plugin can't grow the registry
// without limit: at most this many distinct keys per instance, and each key /
// value truncated to this many bytes before storage. A `ttl_ms == 0` signal is
// sticky, but its retained lifetime is still bounded — `retain_instances` drops
// it when the plugin's pane closes.
const MAX_KEYS_PER_INSTANCE: usize = 16;
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

/// Record a signal pushed by plugin instance `instance_id` (of plugin
/// `plugin_id`). `ttl_ms == 0` is sticky. Over-long keys/values are truncated,
/// and a new key beyond the per-instance cap is dropped; last-write-wins on
/// existing keys is unaffected.
pub fn emit(
    instance_id: &str,
    plugin_id: &str,
    key: String,
    value: String,
    status: SignalStatus,
    ttl_ms: u32,
) {
    let key = truncate_bytes(&key, MAX_KEY_LEN);
    let value = truncate_bytes(&value, MAX_VALUE_LEN);
    let expires_at = (ttl_ms > 0).then(|| Instant::now() + Duration::from_millis(ttl_ms as u64));
    if let Ok(mut reg) = REGISTRY.lock() {
        let id = (instance_id.to_string(), key.clone());
        if !reg.map.contains_key(&id) {
            // Cap distinct keys per instance; drop new keys past the limit.
            let keys_for_instance = reg.map.keys().filter(|(iid, _)| iid == instance_id).count();
            if keys_for_instance >= MAX_KEYS_PER_INSTANCE {
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
                plugin_id: plugin_id.to_string(),
                expires_at,
            },
        );
    }
}

/// Live (non-expired) signals grouped by instance, in stable insertion order.
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

    // Group by instance, preserving first-seen order for both instances and keys.
    let mut groups: Vec<PluginSignals> = Vec::new();
    for id in &reg.order {
        let Some(sig) = reg.map.get(id) else { continue };
        let (instance_id, _) = id;
        match groups.iter_mut().find(|g| &g.instance_id == instance_id) {
            Some(g) => g.signals.push(sig.clone()),
            None => groups.push(PluginSignals {
                instance_id: instance_id.clone(),
                plugin_id: sig.plugin_id.clone(),
                signals: vec![sig.clone()],
            }),
        }
    }
    groups
}

/// Drop signals for every plugin instance **not** in `open`. Called each frame
/// with the set of instance ids that still have a live pane, so a closed pane's
/// signals stop showing in the status bar. Windows are separate OS processes,
/// so this process's `open` set is authoritative for its own registry.
pub fn retain_instances(open: &std::collections::HashSet<String>) {
    if let Ok(mut reg) = REGISTRY.lock() {
        reg.map.retain(|(iid, _), _| open.contains(iid));
        reg.order.retain(|(iid, _)| open.contains(iid));
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
        emit(
            "p#1",
            "p",
            "rows".into(),
            "1".into(),
            SignalStatus::Loading,
            0,
        );
        emit(
            "p#1",
            "p",
            "rows".into(),
            "42".into(),
            SignalStatus::Ready,
            0,
        );
        let snap = snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].signals.len(), 1);
        assert_eq!(snap[0].signals[0].value, "42");
        assert_eq!(snap[0].signals[0].status, SignalStatus::Ready);
    }

    #[test]
    fn same_plugin_different_instances_stay_separate() {
        let _guard = reset();
        // Two tabs of the same plugin must not clobber each other.
        emit(
            "seshat#1",
            "seshat",
            "rows".into(),
            "10".into(),
            SignalStatus::Ready,
            0,
        );
        emit(
            "seshat#2",
            "seshat",
            "rows".into(),
            "20".into(),
            SignalStatus::Ready,
            0,
        );
        let snap = snapshot();
        assert_eq!(snap.len(), 2);
        assert_eq!(snap[0].instance_id, "seshat#1");
        assert_eq!(snap[0].plugin_id, "seshat");
        assert_eq!(snap[0].signals[0].value, "10");
        assert_eq!(snap[1].instance_id, "seshat#2");
        assert_eq!(snap[1].signals[0].value, "20");
    }

    #[test]
    fn retain_instances_drops_closed() {
        let _guard = reset();
        emit("a#1", "a", "x".into(), "1".into(), SignalStatus::Ready, 0);
        emit("b#1", "b", "y".into(), "2".into(), SignalStatus::Ready, 0);
        let open = std::collections::HashSet::from(["b#1".to_string()]);
        retain_instances(&open);
        let snap = snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].instance_id, "b#1");
    }

    #[test]
    fn caps_keys_per_instance_and_truncates() {
        let _guard = reset();
        // One extra key beyond the cap is dropped.
        for i in 0..(MAX_KEYS_PER_INSTANCE + 5) {
            emit(
                "p#1",
                "p",
                format!("k{i}"),
                "v".into(),
                SignalStatus::Ready,
                0,
            );
        }
        let snap = snapshot();
        assert_eq!(snap[0].signals.len(), MAX_KEYS_PER_INSTANCE);
        // Over-long value is truncated to the byte cap.
        emit(
            "q#1",
            "q",
            "big".into(),
            "x".repeat(1000),
            SignalStatus::Ready,
            0,
        );
        let snap = snapshot();
        let q = snap.iter().find(|g| g.instance_id == "q#1").unwrap();
        assert_eq!(q.signals[0].value.len(), MAX_VALUE_LEN);
    }
}
