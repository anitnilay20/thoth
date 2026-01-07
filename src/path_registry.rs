/// Platform-specific PATH registration for making `thoth` command available in CLI
///
/// This module handles adding the Thoth binary to the system PATH on different platforms:
/// - macOS/Linux: Creates/modifies shell profile files (.zshrc, .bashrc, etc.)
/// - Windows: Modifies User PATH environment variable via registry
use crate::error::{Result, ThothError};
use std::env;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use std::fs::OpenOptions;
#[cfg(target_os = "macos")]
use std::io::{BufRead, BufReader, Write};

#[cfg(target_os = "windows")]
use std::process::Command;

/// Check if Thoth is already available in PATH
pub fn is_in_path() -> bool {
    // Try to find 'thoth' in PATH
    which::which("thoth").is_ok()
}

/// Get the path to the Thoth executable
fn get_executable_path() -> Result<PathBuf> {
    env::current_exe().map_err(|e| ThothError::PathRegistryError {
        reason: format!("Failed to get executable path: {}", e),
    })
}

/// Get the directory containing the Thoth executable
fn get_executable_dir() -> Result<PathBuf> {
    let exe_path = get_executable_path()?;
    exe_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| ThothError::PathRegistryError {
            reason: "Failed to get executable directory".to_string(),
        })
}

/// Register Thoth in system PATH
#[cfg(target_os = "macos")]
pub fn register_in_path() -> Result<()> {
    let exe_dir = get_executable_dir()?;
    let exe_dir_str = exe_dir.to_string_lossy();

    // Determine which shell profile to update
    let home_dir = dirs::home_dir().ok_or_else(|| ThothError::PathRegistryError {
        reason: "Failed to get home directory".to_string(),
    })?;

    // Check for .zshrc (default on macOS Catalina+)
    let zshrc_path = home_dir.join(".zshrc");
    let profile_path = if zshrc_path.exists() {
        zshrc_path
    } else {
        // Fall back to .bash_profile
        home_dir.join(".bash_profile")
    };

    // Check if PATH is already added
    if profile_path.exists() {
        let file =
            std::fs::File::open(&profile_path).map_err(|e| ThothError::PathRegistryError {
                reason: format!("Failed to read shell profile: {}", e),
            })?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            if let Ok(line) = line {
                if line.contains(&*exe_dir_str) && line.contains("PATH") {
                    // Already registered
                    return Ok(());
                }
            }
        }
    }

    // Append PATH export to profile
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&profile_path)
        .map_err(|e| ThothError::PathRegistryError {
            reason: format!("Failed to open shell profile for writing: {}", e),
        })?;

    let export_line = format!(
        "\n# Added by Thoth\nexport PATH=\"$PATH:{}\"\n",
        exe_dir_str
    );
    file.write_all(export_line.as_bytes())
        .map_err(|e| ThothError::PathRegistryError {
            reason: format!("Failed to write to shell profile: {}", e),
        })?;

    Ok(())
}

/// Register Thoth in system PATH
#[cfg(target_os = "linux")]
pub fn register_in_path() -> Result<()> {
    let exe_dir = get_executable_dir()?;
    let exe_dir_str = exe_dir.to_string_lossy();

    // Determine which shell profile to update
    let home_dir = dirs::home_dir().ok_or_else(|| ThothError::PathRegistryError {
        reason: "Failed to get home directory".to_string(),
    })?;

    // Check for .bashrc (most common on Linux)
    let bashrc_path = home_dir.join(".bashrc");
    let profile_path = if bashrc_path.exists() {
        bashrc_path
    } else {
        // Fall back to .profile
        home_dir.join(".profile")
    };

    // Check if PATH is already added
    if profile_path.exists() {
        let file =
            std::fs::File::open(&profile_path).map_err(|e| ThothError::PathRegistryError {
                reason: format!("Failed to read shell profile: {}", e),
            })?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            if let Ok(line) = line {
                if line.contains(&*exe_dir_str) && line.contains("PATH") {
                    // Already registered
                    return Ok(());
                }
            }
        }
    }

    // Append PATH export to profile
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&profile_path)
        .map_err(|e| ThothError::PathRegistryError {
            reason: format!("Failed to open shell profile for writing: {}", e),
        })?;

    let export_line = format!(
        "\n# Added by Thoth\nexport PATH=\"$PATH:{}\"\n",
        exe_dir_str
    );
    file.write_all(export_line.as_bytes())
        .map_err(|e| ThothError::PathRegistryError {
            reason: format!("Failed to write to shell profile: {}", e),
        })?;

    Ok(())
}

/// Register Thoth in system PATH
#[cfg(target_os = "windows")]
pub fn register_in_path() -> Result<()> {
    let exe_dir = get_executable_dir()?;
    let exe_dir_str = exe_dir.to_string_lossy();

    // Use PowerShell to modify User PATH environment variable
    let powershell_script = format!(
        r#"
        $currentPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        if ($currentPath -notlike '*{}*') {{
            $newPath = $currentPath + ';{}'
            [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
            Write-Output 'SUCCESS'
        }} else {{
            Write-Output 'ALREADY_EXISTS'
        }}
        "#,
        exe_dir_str, exe_dir_str
    );

    let output = Command::new("powershell")
        .args(&["-Command", &powershell_script])
        .output()
        .map_err(|e| ThothError::PathRegistryError {
            reason: format!("Failed to execute PowerShell: {}", e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ThothError::PathRegistryError {
            reason: format!("PowerShell command failed: {}", stderr),
        });
    }

    Ok(())
}

/// Unregister Thoth from system PATH
#[cfg(target_os = "macos")]
pub fn unregister_from_path() -> Result<()> {
    let exe_dir = get_executable_dir()?;
    let exe_dir_str = exe_dir.to_string_lossy();

    let home_dir = dirs::home_dir().ok_or_else(|| ThothError::PathRegistryError {
        reason: "Failed to get home directory".to_string(),
    })?;

    // Check both .zshrc and .bash_profile
    let profiles = vec![home_dir.join(".zshrc"), home_dir.join(".bash_profile")];

    for profile_path in profiles {
        if !profile_path.exists() {
            continue;
        }

        // Read all lines
        let file =
            std::fs::File::open(&profile_path).map_err(|e| ThothError::PathRegistryError {
                reason: format!("Failed to read shell profile: {}", e),
            })?;
        let reader = BufReader::new(file);
        let mut lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();

        // Remove lines containing the Thoth PATH
        let original_len = lines.len();
        lines.retain(|line| !line.contains(&*exe_dir_str) || !line.contains("PATH"));

        // Also remove "# Added by Thoth" comment if it's on the line before
        let mut i = 0;
        while i < lines.len() {
            if lines[i].contains("# Added by Thoth") {
                lines.remove(i);
            } else {
                i += 1;
            }
        }

        // Only write if something changed
        if lines.len() != original_len {
            std::fs::write(&profile_path, lines.join("\n") + "\n").map_err(|e| {
                ThothError::PathRegistryError {
                    reason: format!("Failed to write shell profile: {}", e),
                }
            })?;
        }
    }

    Ok(())
}

/// Unregister Thoth from system PATH
#[cfg(target_os = "linux")]
pub fn unregister_from_path() -> Result<()> {
    let exe_dir = get_executable_dir()?;
    let exe_dir_str = exe_dir.to_string_lossy();

    let home_dir = dirs::home_dir().ok_or_else(|| ThothError::PathRegistryError {
        reason: "Failed to get home directory".to_string(),
    })?;

    // Check both .bashrc and .profile
    let profiles = vec![home_dir.join(".bashrc"), home_dir.join(".profile")];

    for profile_path in profiles {
        if !profile_path.exists() {
            continue;
        }

        // Read all lines
        let file =
            std::fs::File::open(&profile_path).map_err(|e| ThothError::PathRegistryError {
                reason: format!("Failed to read shell profile: {}", e),
            })?;
        let reader = BufReader::new(file);
        let mut lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();

        // Remove lines containing the Thoth PATH
        let original_len = lines.len();
        lines.retain(|line| !line.contains(&*exe_dir_str) || !line.contains("PATH"));

        // Also remove "# Added by Thoth" comment
        let mut i = 0;
        while i < lines.len() {
            if lines[i].contains("# Added by Thoth") {
                lines.remove(i);
            } else {
                i += 1;
            }
        }

        // Only write if something changed
        if lines.len() != original_len {
            std::fs::write(&profile_path, lines.join("\n") + "\n").map_err(|e| {
                ThothError::PathRegistryError {
                    reason: format!("Failed to write shell profile: {}", e),
                }
            })?;
        }
    }

    Ok(())
}

/// Unregister Thoth from system PATH
#[cfg(target_os = "windows")]
pub fn unregister_from_path() -> Result<()> {
    let exe_dir = get_executable_dir()?;
    let exe_dir_str = exe_dir.to_string_lossy();

    // Use PowerShell to remove from User PATH
    let powershell_script = format!(
        r#"
        $currentPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        $newPath = ($currentPath -split ';' | Where-Object {{ $_ -ne '{}' }}) -join ';'
        [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
        Write-Output 'SUCCESS'
        "#,
        exe_dir_str
    );

    let output = Command::new("powershell")
        .args(&["-Command", &powershell_script])
        .output()
        .map_err(|e| ThothError::PathRegistryError {
            reason: format!("Failed to execute PowerShell: {}", e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ThothError::PathRegistryError {
            reason: format!("PowerShell command failed: {}", stderr),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_executable_path() {
        let result = get_executable_path();
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_executable_dir() {
        let result = get_executable_dir();
        assert!(result.is_ok());
        if let Ok(dir) = result {
            assert!(dir.is_dir() || dir.to_string_lossy().contains("target"));
        }
    }

    #[test]
    fn test_is_in_path() {
        // This test just checks that the function doesn't panic
        let _ = is_in_path();
    }
}
