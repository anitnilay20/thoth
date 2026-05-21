/// Platform-specific PATH registration for making `thoth` command available in CLI
///
/// - macOS: Symlink at `/usr/local/bin/thoth`. Tries direct creation first;
///   falls back to `osascript` admin dialog (same as VS Code "Install
///   'code' command in PATH") when the directory is root-owned.
/// - Linux: Symlink at `~/.local/bin/thoth` — XDG standard, no root needed,
///   on PATH by default on Ubuntu 20.04+, Fedora, Arch, etc.
/// - Windows: Modifies the User PATH registry key via PowerShell.
use crate::error::{Result, ThothError};
use std::env;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use std::process::Command;

/// Check if Thoth is already registered.
///
/// On Unix we check for the well-known symlink directly so the status updates
/// immediately in the same session (no shell restart, no process-env check).
pub fn is_in_path() -> bool {
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/usr/local/bin/thoth").exists()
    }
    #[cfg(target_os = "linux")]
    {
        local_bin_link().map(|p| p.exists()).unwrap_or(false)
    }
    #[cfg(target_os = "windows")]
    {
        which::which("thoth").is_ok()
    }
}

fn get_executable_path() -> Result<PathBuf> {
    env::current_exe().map_err(|e| ThothError::PathRegistryError {
        reason: format!("Failed to get executable path: {}", e),
    })
}

// ── macOS ─────────────────────────────────────────────────────────────────────

/// Escapes single quotes so a path can be safely embedded in a single-quoted
/// POSIX shell string (`'…'`).  Each `'` becomes `'\''` (close quote, escaped
/// literal, reopen quote).
#[cfg(target_os = "macos")]
fn shell_escape_single_quoted(s: &str) -> String {
    s.replace('\'', "'\\''")
}

/// Attempts a direct `symlink()` call without privilege elevation.
/// Returns `Ok(())` on success; callers should inspect the error kind
/// (e.g. `ErrorKind::PermissionDenied`) to decide whether to retry with
/// elevated privileges.
#[cfg(target_os = "macos")]
fn try_symlink_direct(exe: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::symlink;
    if let Some(parent) = link.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;
    }
    if link.symlink_metadata().is_ok() {
        std::fs::remove_file(link)?;
    }
    symlink(exe, link)
}

#[cfg(target_os = "macos")]
pub fn register_in_path() -> Result<()> {
    use std::process::Command;

    let exe_path = get_executable_path()?;
    let link_path = PathBuf::from("/usr/local/bin/thoth");

    // Happy path: /usr/local/bin writable without auth (e.g. Homebrew setups).
    if try_symlink_direct(&exe_path, &link_path).is_ok() {
        return Ok(());
    }

    // Fall back: native macOS admin dialog via osascript — identical UX to
    // VS Code "Install 'code' command in PATH".
    let exe_escaped = shell_escape_single_quoted(&exe_path.to_string_lossy());
    let script = format!(
        "do shell script \"mkdir -p /usr/local/bin && ln -sf '{}' '/usr/local/bin/thoth'\" with administrator privileges",
        exe_escaped
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| ThothError::PathRegistryError {
            reason: format!("Failed to launch authorization dialog: {}", e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ThothError::PathRegistryError {
            reason: format!("Could not install shell command: {}", stderr.trim()),
        });
    }

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn unregister_from_path() -> Result<()> {
    use std::process::Command;

    let link_path = PathBuf::from("/usr/local/bin/thoth");
    if link_path.symlink_metadata().is_err() {
        return Ok(());
    }

    // Try direct removal first.
    if std::fs::remove_file(&link_path).is_ok() {
        return Ok(());
    }

    // Needs privilege elevation.
    let output = Command::new("osascript")
        .args([
            "-e",
            "do shell script \"rm -f '/usr/local/bin/thoth'\" with administrator privileges",
        ])
        .output()
        .map_err(|e| ThothError::PathRegistryError {
            reason: format!("Failed to launch authorization dialog: {}", e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ThothError::PathRegistryError {
            reason: format!("Could not remove shell command: {}", stderr.trim()),
        });
    }

    Ok(())
}

// ── Linux ─────────────────────────────────────────────────────────────────────

/// `~/.local/bin/thoth` — XDG convention, no root required.
#[cfg(target_os = "linux")]
fn local_bin_link() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| ThothError::PathRegistryError {
        reason: "Cannot determine home directory".to_string(),
    })?;
    Ok(home.join(".local/bin/thoth"))
}

#[cfg(target_os = "linux")]
pub fn register_in_path() -> Result<()> {
    use std::os::unix::fs::symlink;

    let exe_path = get_executable_path()?;
    let link_path = local_bin_link()?;

    if let Some(parent) = link_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| ThothError::PathRegistryError {
                reason: format!("Failed to create ~/.local/bin: {}", e),
            })?;
        }
    }

    if link_path.symlink_metadata().is_ok() {
        std::fs::remove_file(&link_path).map_err(|e| ThothError::PathRegistryError {
            reason: format!("Failed to remove existing symlink: {}", e),
        })?;
    }

    symlink(&exe_path, &link_path).map_err(|e| ThothError::PathRegistryError {
        reason: format!("Failed to create symlink at ~/.local/bin/thoth: {}", e),
    })?;

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn unregister_from_path() -> Result<()> {
    let link_path = local_bin_link()?;
    if link_path.symlink_metadata().is_ok() {
        std::fs::remove_file(&link_path).map_err(|e| ThothError::PathRegistryError {
            reason: format!("Failed to remove ~/.local/bin/thoth: {}", e),
        })?;
    }
    Ok(())
}

// ── Windows ───────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn get_executable_dir() -> Result<PathBuf> {
    get_executable_path()?
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| ThothError::PathRegistryError {
            reason: "Failed to get executable directory".to_string(),
        })
}

#[cfg(target_os = "windows")]
pub fn register_in_path() -> Result<()> {
    let exe_dir_str = get_executable_dir()?.to_string_lossy().into_owned();

    // Split on ';' and compare each trimmed segment exactly to avoid false
    // positives from substring matches (e.g. C:\Foo\thoth matching C:\thoth).
    let script = format!(
        r#"
        $currentPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        $segments = $currentPath -split ';' | ForEach-Object {{ $_.Trim() }}
        if ($segments -notcontains '{0}') {{
            [Environment]::SetEnvironmentVariable('Path', $currentPath + ';{0}', 'User')
        }}
        "#,
        exe_dir_str
    );

    let output = Command::new("powershell")
        .args(["-Command", &script])
        .output()
        .map_err(|e| ThothError::PathRegistryError {
            reason: format!("Failed to execute PowerShell: {}", e),
        })?;

    if !output.status.success() {
        return Err(ThothError::PathRegistryError {
            reason: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn unregister_from_path() -> Result<()> {
    let exe_dir_str = get_executable_dir()?.to_string_lossy().into_owned();

    let script = format!(
        r#"
        $currentPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        $newPath = ($currentPath -split ';' | Where-Object {{ $_ -ne '{0}' }}) -join ';'
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
        "#,
        exe_dir_str
    );

    let output = Command::new("powershell")
        .args(["-Command", &script])
        .output()
        .map_err(|e| ThothError::PathRegistryError {
            reason: format!("Failed to execute PowerShell: {}", e),
        })?;

    if !output.status.success() {
        return Err(ThothError::PathRegistryError {
            reason: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_executable_path() {
        assert!(get_executable_path().is_ok());
    }

    #[test]
    fn test_is_in_path_does_not_panic() {
        let _ = is_in_path();
    }
}
