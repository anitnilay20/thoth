/// Platform abstraction layer for cross-platform operations
///
/// This module provides platform-agnostic interfaces for operations
/// that have different implementations on Unix and Windows systems.
///
/// # Example
/// ```no_run
/// use std::fs::File;
/// use thoth::platform::FileIO;
///
/// let file = File::open("data.json")?;
/// let mut buffer = vec![0u8; 1024];
/// file.read_at(&mut buffer, 0)?; // Works on both Unix and Windows
/// # Ok::<(), std::io::Error>(())
/// ```
pub mod archive;
pub mod file_io;
pub mod file_open_channel;
pub mod fonts;
pub mod fs;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod native_menu;
pub mod path_registry;

pub use archive::get_extractor_for_file;
pub use file_io::FileIO;
pub use file_open_channel::{drain_open_requests, enqueue_open_request};
pub use fonts::{find_font_bytes, find_font_bytes_weighted, has_weight, list_system_font_families};
pub use fs::get_fs_ops;

/// Return the real executable path used to anchor bundled resources and PATH
/// registrations. Unix launchers commonly invoke Thoth through a symlink; the
/// unresolved launcher path does not live next to the application resources.
pub(crate) fn current_executable_path() -> std::io::Result<std::path::PathBuf> {
    resolve_executable_path(&std::env::current_exe()?)
}

fn resolve_executable_path(path: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    #[cfg(unix)]
    {
        path.canonicalize()
    }
    #[cfg(not(unix))]
    {
        Ok(path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[test]
    fn executable_path_resolves_symlinks() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("Thoth.app/Contents/MacOS/thoth");
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        std::fs::write(&executable, b"binary").unwrap();
        let launcher = dir.path().join("thoth");
        symlink(&executable, &launcher).unwrap();

        assert_eq!(
            super::resolve_executable_path(&launcher).unwrap(),
            executable.canonicalize().unwrap()
        );
    }
}
