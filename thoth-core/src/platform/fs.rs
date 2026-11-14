/// Cross-platform filesystem operations
///
/// Provides platform-specific file operations like setting executable permissions
use crate::error::Result;
use std::path::Path;

pub trait FileSystemOps {
    /// Make a file executable
    fn make_executable(&self, path: &Path) -> Result<()>;
}

pub struct PlatformFs;

impl FileSystemOps for PlatformFs {
    #[cfg(unix)]
    fn make_executable(&self, path: &Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms)?;
        Ok(())
    }

    #[cfg(not(unix))]
    fn make_executable(&self, _path: &Path) -> Result<()> {
        // On Windows, executables don't need special permissions
        Ok(())
    }
}

/// Get the platform-specific filesystem operations
pub fn get_fs_ops() -> Box<dyn FileSystemOps> {
    Box::new(PlatformFs)
}
