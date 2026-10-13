//! Process-global synchronisation for tests in this crate.
//!
//! The Papyrus registry and the index cache are process-wide (they live in
//! `thoth-core`). Tests that publish to the registry or act on the whole cache
//! directory take the matching lock, and anything that writes a cache entry
//! points the cache at a scratch directory via `THOTH_INDEX_CACHE_DIR`.

use std::sync::{Mutex, MutexGuard, OnceLock};

static CACHE_LOCK: Mutex<()> = Mutex::new(());
static REGISTRY_LOCK: Mutex<()> = Mutex::new(());

/// Serialise tests that act on the whole index-cache directory.
pub fn cache_lock() -> MutexGuard<'static, ()> {
    CACHE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Serialise tests that publish to the Papyrus registry.
pub fn registry_lock() -> MutexGuard<'static, ()> {
    REGISTRY_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Point the index cache at a scratch directory for the whole test binary, so a
/// test run can never disturb the user's cache.
pub fn isolate_cache() {
    static SCRATCH: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = SCRATCH.get_or_init(|| tempfile::tempdir().expect("scratch cache dir"));
    // SAFETY: tests in this binary only ever set this to the same value.
    unsafe { std::env::set_var(thoth_core::file::index_cache::CACHE_DIR_ENV, dir.path()) };
}
