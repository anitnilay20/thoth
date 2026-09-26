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
    resolve_root(std::env::var_os(CONFIG_DIR_ENV))
}

/// The root an override value resolves to, with the environment factored out
/// so the rule can be tested without a test writing a process-global variable
/// that every other test reads.
fn resolve_root(override_dir: Option<std::ffi::OsString>) -> Option<PathBuf> {
    if let Some(dir) = override_dir.filter(|d| !d.is_empty()) {
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
    SCRATCH.get_or_init(|| {
        let dir = tempfile::tempdir().expect("scratch config dir");
        // SAFETY: runs exactly once per process, and `OnceLock` blocks every
        // other caller until it has finished. Writing on each call instead
        // would race a thread already reading it — Rust's env lock covers std
        // readers, and bundled C code calling `getenv` is not one of them.
        unsafe { std::env::set_var(CONFIG_DIR_ENV, dir.path()) };
        dir
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_override_replaces_the_platform_directory() {
        assert_eq!(
            resolve_root(Some("/tmp/thoth-root-test".into())),
            Some(PathBuf::from("/tmp/thoth-root-test"))
        );
    }

    #[test]
    fn an_empty_override_is_not_an_override() {
        // It would otherwise point the whole app at the process's working
        // directory, which is wherever the user happened to launch it.
        assert_eq!(
            resolve_root(Some("".into())),
            dirs::config_dir().map(|d| d.join("thoth")),
        );
        assert_eq!(
            resolve_root(None),
            dirs::config_dir().map(|d| d.join("thoth")),
        );
    }
}
