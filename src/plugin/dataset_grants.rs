//! Session-scoped consent grants for cross-plugin dataset reads (#116).
//!
//! A consumer plugin reading a *different* producer's rows is gated: the host
//! raises a consent prompt on the first attempt and records the approved
//! `(consumer plugin, source plugin)` pair here for the rest of the session.
//! Reading a plugin's own datasets is never gated. Grants are in-memory only —
//! they reset on restart, so access is re-confirmed each session.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

/// Approved `(consumer_plugin_id, source_plugin_id)` pairs.
static GRANTS: LazyLock<Mutex<HashSet<(String, String)>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));
/// Pairs with a consent prompt already in flight — so a consumer re-reading
/// while awaiting approval doesn't stack duplicate modals.
static PENDING: LazyLock<Mutex<HashSet<(String, String)>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// Whether `consumer` may read `source`'s datasets this session.
pub fn is_granted(consumer: &str, source: &str) -> bool {
    GRANTS
        .lock()
        .map(|g| g.contains(&(consumer.to_string(), source.to_string())))
        .unwrap_or(false)
}

/// Record an approved pair (and clear any pending marker).
pub fn grant(consumer: &str, source: &str) {
    if let Ok(mut g) = GRANTS.lock() {
        g.insert((consumer.to_string(), source.to_string()));
    }
    clear_pending(consumer, source);
}

/// Mark a consent prompt as in flight; returns `true` if this call is the one
/// that should raise the prompt (i.e. none was already pending).
pub fn mark_pending(consumer: &str, source: &str) -> bool {
    PENDING
        .lock()
        .map(|mut p| p.insert((consumer.to_string(), source.to_string())))
        .unwrap_or(false)
}

/// Drop the pending marker (on approve or deny), so a later read can re-prompt.
pub fn clear_pending(consumer: &str, source: &str) {
    if let Ok(mut p) = PENDING.lock() {
        p.remove(&(consumer.to_string(), source.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_and_pending_lifecycle() {
        assert!(!is_granted("c", "s"));
        // First mark raises; second (while pending) does not.
        assert!(mark_pending("c", "s"));
        assert!(!mark_pending("c", "s"));
        grant("c", "s");
        assert!(is_granted("c", "s"));
        // Granting cleared the pending marker, so a fresh prompt could raise again.
        assert!(mark_pending("c", "s"));
    }
}
