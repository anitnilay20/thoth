//! Where Thoth keeps everything it owns on disk.
//!
//! Every path the app persists to — settings, recent files, bookmarks, open
//! tabs, search history, plugin state, the marketplace, the index cache —
//! hangs off one root, and that root can be moved with a single environment
//! variable.
//!
//! That override is the point. A test that writes to the real root does not
//! merely add to the user's data, it *replaces* it: state is saved as a whole
//! document, so a run that opens one temp file leaves the user with a recent
//! list one entry long. One root means a test moves all of it at once and
//! cannot miss a corner.

use std::path::PathBuf;

/// Relocates everything Thoth stores. Set by tests so a run can never touch
/// the user's real state, and usable operationally to move the whole lot —
/// onto a different volume, or to keep several installs apart.
pub const CONFIG_DIR_ENV: &str = "THOTH_CONFIG_DIR";

/// Thoth's directory under the user's config dir, or whatever
/// [`CONFIG_DIR_ENV`] names.
///
/// Resolves a path and nothing more: callers create what they need and report
/// failure in their own terms. `None` only when the platform has no config
/// directory *and* no override is set.
pub fn config_root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(CONFIG_DIR_ENV).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    Some(dirs::config_dir()?.join("thoth"))
}

/// Point the whole config root at a scratch directory for the rest of this
/// process, so nothing it does can reach the user's real state.
///
/// Call it before anything loads or saves. Idempotent within a process: the
/// scratch directory is created once and every later call names the same one,
/// so tests sharing a binary can each call it without racing.
///
/// Lives outside `#[cfg(test)]` because the integration tests are separate
/// crates and a `#[cfg(test)]` helper cannot reach them — which is exactly how
/// `tests/file_association_tests.rs` came to write the user's recent files.
#[doc(hidden)]
pub fn isolate_for_tests() {
    use std::sync::OnceLock;
    static SCRATCH: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = SCRATCH.get_or_init(|| tempfile::tempdir().expect("scratch config dir"));
    // SAFETY: every caller in a process sets this to the same value, and the
    // first call happens before any thread reads it.
    unsafe { std::env::set_var(CONFIG_DIR_ENV, dir.path()) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_override_replaces_the_platform_directory() {
        // Serialised against the other env-touching test below by running the
        // whole check under one lock-free sequence: both only read/write this
        // one variable and restore it.
        let before = std::env::var_os(CONFIG_DIR_ENV);

        // SAFETY: test-local, restored below.
        unsafe { std::env::set_var(CONFIG_DIR_ENV, "/tmp/thoth-root-test") };
        assert_eq!(config_root(), Some(PathBuf::from("/tmp/thoth-root-test")));

        // An empty value is not an override — it would silently point the app
        // at the process's working directory.
        unsafe { std::env::set_var(CONFIG_DIR_ENV, "") };
        assert_eq!(
            config_root(),
            dirs::config_dir().map(|d| d.join("thoth")),
            "an empty override should fall back to the platform directory"
        );

        match before {
            Some(v) => unsafe { std::env::set_var(CONFIG_DIR_ENV, v) },
            None => unsafe { std::env::remove_var(CONFIG_DIR_ENV) },
        }
    }
}
