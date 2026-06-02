use super::types::ReleaseInfo;
use crate::error::{Result, ThothError};
use crate::platform::{get_extractor_for_file, get_fs_ops};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

const GITHUB_REPO: &str = "anitnilay20/thoth";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub enum UpdateMessage {
    UpdateCheckComplete(Result<Vec<ReleaseInfo>>),
    DownloadProgress(f32),
    DownloadComplete(Result<std::path::PathBuf>),
    InstallComplete(Result<()>),
}

pub struct UpdateManager {
    tx: Sender<UpdateMessage>,
    rx: Receiver<UpdateMessage>,
}

impl Default for UpdateManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateManager {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { tx, rx }
    }

    pub fn receiver(&mut self) -> &mut Receiver<UpdateMessage> {
        &mut self.rx
    }

    pub fn check_for_updates(&self) {
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = Self::fetch_releases();
            let msg = match result {
                Ok(releases) => UpdateMessage::UpdateCheckComplete(Ok(releases)),
                Err(e) => UpdateMessage::UpdateCheckComplete(Err(e)),
            };
            let _ = tx.send(msg);
        });
    }

    fn fetch_releases() -> Result<Vec<ReleaseInfo>> {
        let url = format!("https://api.github.com/repos/{}/releases", GITHUB_REPO);

        let client = reqwest::blocking::Client::builder()
            .user_agent("thoth-updater")
            .build()?;

        let response = client
            .get(&url)
            .send()
            .map_err(|e| ThothError::UpdateCheckError {
                reason: format!("Failed to fetch releases from GitHub: {}", e),
            })?;

        if !response.status().is_success() {
            return Err(ThothError::UpdateCheckError {
                reason: format!("GitHub API returned status: {}", response.status()),
            });
        }

        let releases: Vec<ReleaseInfo> =
            response.json().map_err(|e| ThothError::UpdateCheckError {
                reason: format!("Failed to parse GitHub releases: {}", e),
            })?;

        Ok(releases)
    }

    pub fn has_newer_version(releases: &[ReleaseInfo]) -> bool {
        if let Some(latest) = releases.first() {
            let latest_version = Self::parse_version(&latest.tag_name);
            let current_version = Self::parse_version(CURRENT_VERSION);

            if let (Some(latest), Some(current)) = (latest_version, current_version) {
                return Self::compare_versions(&latest, &current) > 0;
            }
        }
        false
    }

    pub fn get_newer_releases(releases: &[ReleaseInfo]) -> Vec<ReleaseInfo> {
        let current_version = Self::parse_version(CURRENT_VERSION);

        releases
            .iter()
            .filter(|release| {
                let release_version = Self::parse_version(&release.tag_name);
                if let (Some(rv), Some(cv)) = (release_version, current_version.as_ref()) {
                    Self::compare_versions(&rv, cv) > 0
                } else {
                    false
                }
            })
            .cloned()
            .collect()
    }

    fn parse_version(version: &str) -> Option<(u32, u32, u32)> {
        let version = version.trim_start_matches('v');
        let parts: Vec<&str> = version.split('.').collect();

        if parts.len() != 3 {
            return None;
        }

        let major = parts[0].parse::<u32>().ok()?;
        let minor = parts[1].parse::<u32>().ok()?;
        let patch = parts[2].parse::<u32>().ok()?;

        Some((major, minor, patch))
    }

    fn compare_versions(a: &(u32, u32, u32), b: &(u32, u32, u32)) -> i32 {
        if a.0 != b.0 {
            return (a.0 as i32) - (b.0 as i32);
        }
        if a.1 != b.1 {
            return (a.1 as i32) - (b.1 as i32);
        }
        (a.2 as i32) - (b.2 as i32)
    }

    pub fn download_update(&self, release: &ReleaseInfo) {
        let tx = self.tx.clone();
        let release = release.clone();

        thread::spawn(move || {
            let result = Self::download_release(&release, &tx);
            let msg = match result {
                Ok(path) => UpdateMessage::DownloadComplete(Ok(path)),
                Err(e) => UpdateMessage::DownloadComplete(Err(e)),
            };
            let _ = tx.send(msg);
        });
    }

    fn download_release(
        release: &ReleaseInfo,
        tx: &Sender<UpdateMessage>,
    ) -> Result<std::path::PathBuf> {
        use std::io::{Read, Write};

        // Determine the correct asset based on platform
        let asset = Self::get_platform_asset(release)?;

        let client = reqwest::blocking::Client::builder()
            .user_agent("thoth-updater")
            .build()?;

        // Create temp directory for download
        let temp_dir = std::env::temp_dir().join("thoth_update");
        std::fs::create_dir_all(&temp_dir)?;

        let file_path = temp_dir.join(&asset.name);

        let mut response = client
            .get(&asset.browser_download_url)
            .send()
            .map_err(|e| ThothError::UpdateDownloadError {
                version: release.tag_name.clone(),
                reason: format!("Failed to download update: {}", e),
            })?;

        if !response.status().is_success() {
            return Err(ThothError::UpdateDownloadError {
                version: release.tag_name.clone(),
                reason: format!("Download failed with status: {}", response.status()),
            });
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut file =
            std::fs::File::create(&file_path).map_err(|e| ThothError::FileWriteError {
                path: file_path.clone(),
                reason: format!("Failed to create file: {}", e),
            })?;
        let mut downloaded: u64 = 0;

        let mut buffer = vec![0; 8192];
        loop {
            let n = response.read(&mut buffer)?;
            if n == 0 {
                break;
            }

            file.write_all(&buffer[..n])
                .map_err(|e| ThothError::FileWriteError {
                    path: file_path.clone(),
                    reason: format!("Failed to write data: {}", e),
                })?;
            downloaded += n as u64;

            if total_size > 0 {
                let progress = (downloaded as f32 / total_size as f32) * 100.0;
                let _ = tx.send(UpdateMessage::DownloadProgress(progress));
            }
        }

        Ok(file_path)
    }

    fn get_platform_asset(release: &ReleaseInfo) -> Result<super::types::ReleaseAsset> {
        // For OTA updates, use archives as they support automatic binary replacement
        // Installers (DMG, MSI, DEB) are provided for first-time installation only
        let archive_name = if cfg!(target_os = "windows") {
            "thoth-x86_64-pc-windows-msvc.zip"
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                "thoth-aarch64-apple-darwin.tar.gz"
            } else {
                "thoth-x86_64-apple-darwin.tar.gz"
            }
        } else if cfg!(target_os = "linux") {
            "thoth-x86_64-unknown-linux-gnu.tar.gz"
        } else {
            return Err(ThothError::UpdateInstallError {
                reason: "Unsupported platform".to_string(),
            });
        };

        release
            .assets
            .iter()
            .find(|asset| asset.name == archive_name)
            .cloned()
            .ok_or_else(|| ThothError::UpdateDownloadError {
                version: release.tag_name.clone(),
                reason: format!("No asset found for current platform: {}", archive_name),
            })
    }

    pub fn install_update(&self, archive_path: std::path::PathBuf) {
        let tx = self.tx.clone();

        thread::spawn(move || {
            let result = Self::extract_and_install(archive_path);
            let msg = match result {
                Ok(_) => UpdateMessage::InstallComplete(Ok(())),
                Err(e) => UpdateMessage::InstallComplete(Err(e)),
            };
            let _ = tx.send(msg);
        });
    }

    fn extract_and_install(archive_path: std::path::PathBuf) -> Result<()> {
        // Extract archive to temp directory
        let temp_dir = std::env::temp_dir().join("thoth_update_extracted");
        std::fs::create_dir_all(&temp_dir)?;

        // Detect file type and extract using platform abstraction
        let file_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| ThothError::UpdateInstallError {
                reason: "Invalid file name in archive path".to_string(),
            })?;

        let extractor = get_extractor_for_file(file_name)?;
        extractor.extract(&archive_path, &temp_dir)?;

        // Get current executable path
        let current_exe = std::env::current_exe()?;

        // On macOS replace the entire .app bundle so Resources (plugins,
        // assets) are updated alongside the binary — not just the binary.
        #[cfg(target_os = "macos")]
        {
            if let Some(app_bundle) = Self::find_in_tree(&temp_dir, "Thoth.app") {
                // current_exe is …/Thoth.app/Contents/MacOS/thoth
                // installed bundle is three levels up
                if let Some(installed_bundle) = current_exe
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                {
                    Self::copy_dir_all(&app_bundle, installed_bundle)?;
                    return Ok(());
                }
            }
            // Fallthrough: no .app found in archive — replace binary + sync plugins
        }

        // Find the new executable in the extracted files
        let new_exe = Self::find_executable(&temp_dir)?;

        // Replace the current executable
        Self::replace_executable(&new_exe, &current_exe)?;

        // Sync bundled plugins from the extracted archive so OTA updates
        // pick up new/updated plugins, not just the binary.
        Self::sync_plugins(&temp_dir, &current_exe);

        Ok(())
    }

    /// Copy `assets/plugins/` from the extracted archive next to the installed
    /// binary (macOS: `../Resources/assets/plugins/`, others: `assets/plugins/`).
    fn sync_plugins(extracted_dir: &std::path::Path, current_exe: &std::path::Path) {
        // Find the plugins directory inside the extracted archive.
        let src = match Self::find_in_tree(extracted_dir, "plugins") {
            Some(p) if p.is_dir() => p,
            _ => return, // archive has no plugins dir — nothing to do
        };

        // Resolve destination relative to the installed binary.
        let dst = {
            #[cfg(target_os = "macos")]
            {
                // binary is at Thoth.app/Contents/MacOS/thoth
                // plugins go to Thoth.app/Contents/Resources/assets/plugins/
                current_exe
                    .parent() // MacOS/
                    .and_then(|p| p.parent()) // Contents/
                    .map(|p| p.join("Resources/assets/plugins"))
            }
            #[cfg(not(target_os = "macos"))]
            {
                // binary sits next to assets/plugins/ on Linux/Windows
                current_exe
                    .parent()
                    .map(|p| p.join("assets/plugins"))
            }
        };

        let dst = match dst {
            Some(p) => p,
            None => return,
        };

        if let Err(e) = Self::copy_dir_all(&src, &dst) {
            eprintln!("Warning: could not sync plugins during update: {e}");
        }
    }

    /// Recursively copy `src` into `dst`, creating directories as needed.
    fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let dst_path = dst.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                Self::copy_dir_all(&entry.path(), &dst_path)?;
            } else {
                std::fs::copy(entry.path(), dst_path)?;
            }
        }
        Ok(())
    }

    /// Walk `dir` looking for a file or directory named `name`.
    fn find_in_tree(dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return None;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().and_then(|n| n.to_str()) == Some(name) {
                return Some(path);
            }
            if path.is_dir() {
                if let Some(found) = Self::find_in_tree(&path, name) {
                    return Some(found);
                }
            }
        }
        None
    }

    fn find_executable(dir: &std::path::Path) -> Result<std::path::PathBuf> {
        let exe_name = if cfg!(target_os = "windows") {
            "thoth.exe"
        } else {
            "thoth"
        };

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.file_name().and_then(|n| n.to_str()) == Some(exe_name) {
                return Ok(path);
            }

            if path.is_dir()
                && let Ok(found) = Self::find_executable(&path)
            {
                return Ok(found);
            }
        }

        Err(ThothError::UpdateInstallError {
            reason: "Could not find executable in extracted archive".to_string(),
        })
    }

    fn replace_executable(new_exe: &std::path::Path, current_exe: &std::path::Path) -> Result<()> {
        let fs_ops = get_fs_ops();
        fs_ops.make_executable(new_exe)?;

        #[cfg(target_os = "windows")]
        {
            // Windows won't let you write over a running .exe, but it does
            // allow renaming one. Rename the running binary out of the way,
            // then copy the new one into its place. The renamed .old file is
            // cleaned up on the next update or can be left harmlessly.
            let old_path = current_exe.with_extension("exe.old");
            if old_path.exists() {
                let _ = std::fs::remove_file(&old_path);
            }
            std::fs::rename(current_exe, &old_path).map_err(|e| {
                crate::error::ThothError::UpdateInstallError {
                    reason: format!("Could not rename current executable: {e}"),
                }
            })?;
            std::fs::copy(new_exe, current_exe).map_err(|e| {
                // Restore original if copy fails
                let _ = std::fs::rename(&old_path, current_exe);
                crate::error::ThothError::UpdateInstallError {
                    reason: format!("Could not write new executable: {e}"),
                }
            })?;
        }

        #[cfg(not(target_os = "windows"))]
        {
            let backup_path = current_exe.with_extension("backup");
            if backup_path.exists() {
                std::fs::remove_file(&backup_path)?;
            }
            std::fs::copy(current_exe, &backup_path)?;
            std::fs::copy(new_exe, current_exe)?;
        }

        Ok(())
    }

    pub fn get_current_version() -> &'static str {
        CURRENT_VERSION
    }
}
