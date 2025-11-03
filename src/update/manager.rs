use super::types::ReleaseInfo;
use anyhow::{Context, Result};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

const GITHUB_REPO: &str = "anitnilay20/thoth";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub enum UpdateMessage {
    UpdateCheckComplete(Result<Vec<ReleaseInfo>, String>),
    DownloadProgress(f32),
    DownloadComplete(Result<std::path::PathBuf, String>),
    InstallComplete(Result<(), String>),
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
                Err(e) => UpdateMessage::UpdateCheckComplete(Err(e.to_string())),
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
            .context("Failed to fetch releases from GitHub")?;

        if !response.status().is_success() {
            anyhow::bail!("GitHub API returned status: {}", response.status());
        }

        let releases: Vec<ReleaseInfo> =
            response.json().context("Failed to parse GitHub releases")?;

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
                Err(e) => UpdateMessage::DownloadComplete(Err(e.to_string())),
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
            .context("Failed to download update")?;

        if !response.status().is_success() {
            anyhow::bail!("Download failed with status: {}", response.status());
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut file = std::fs::File::create(&file_path)?;
        let mut downloaded: u64 = 0;

        let mut buffer = vec![0; 8192];
        loop {
            let n = response.read(&mut buffer)?;
            if n == 0 {
                break;
            }

            file.write_all(&buffer[..n])?;
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
            anyhow::bail!("Unsupported platform");
        };

        release
            .assets
            .iter()
            .find(|asset| asset.name == archive_name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No asset found for current platform"))
    }

    pub fn install_update(&self, archive_path: std::path::PathBuf) {
        let tx = self.tx.clone();

        thread::spawn(move || {
            let result = Self::extract_and_install(archive_path);
            let msg = match result {
                Ok(_) => UpdateMessage::InstallComplete(Ok(())),
                Err(e) => UpdateMessage::InstallComplete(Err(e.to_string())),
            };
            let _ = tx.send(msg);
        });
    }

    fn extract_and_install(archive_path: std::path::PathBuf) -> Result<()> {
        // Extract archive to temp directory
        let temp_dir = std::env::temp_dir().join("thoth_update_extracted");
        std::fs::create_dir_all(&temp_dir)?;

        // Detect file type and extract
        let file_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;

        if file_name.ends_with(".zip") {
            Self::extract_zip(&archive_path, &temp_dir)?;
        } else if file_name.ends_with(".tar.gz") {
            Self::extract_tar_gz(&archive_path, &temp_dir)?;
        } else {
            anyhow::bail!("Unsupported archive format: {}", file_name);
        }

        // Get current executable path
        let current_exe = std::env::current_exe()?;

        // Find the new executable in the extracted files
        let new_exe = Self::find_executable(&temp_dir)?;

        // Replace the current executable
        Self::replace_executable(&new_exe, &current_exe)?;

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn extract_zip(archive_path: &std::path::Path, dest_dir: &std::path::Path) -> Result<()> {
        use std::io::Read;
        let file = std::fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = dest_dir.join(file.name());

            if file.is_dir() {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn extract_tar_gz(archive_path: &std::path::Path, dest_dir: &std::path::Path) -> Result<()> {
        use flate2::read::GzDecoder;
        use tar::Archive;

        let file = std::fs::File::open(archive_path)?;
        let gz = GzDecoder::new(file);
        let mut archive = Archive::new(gz);
        archive.unpack(dest_dir)?;

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn extract_tar_gz(_archive_path: &std::path::Path, _dest_dir: &std::path::Path) -> Result<()> {
        anyhow::bail!("tar.gz extraction not needed on Windows")
    }

    #[cfg(not(target_os = "windows"))]
    fn extract_zip(_archive_path: &std::path::Path, _dest_dir: &std::path::Path) -> Result<()> {
        anyhow::bail!("zip extraction not needed on Unix")
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

            if path.is_dir() {
                if let Ok(found) = Self::find_executable(&path) {
                    return Ok(found);
                }
            }
        }

        anyhow::bail!("Could not find executable in extracted archive")
    }

    fn replace_executable(new_exe: &std::path::Path, current_exe: &std::path::Path) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Set executable permissions
            let mut perms = std::fs::metadata(new_exe)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(new_exe, perms)?;
        }

        // Create backup of current executable
        let backup_path = current_exe.with_extension("backup");
        if backup_path.exists() {
            std::fs::remove_file(&backup_path)?;
        }
        std::fs::copy(current_exe, &backup_path)?;

        // Replace current executable
        std::fs::copy(new_exe, current_exe)?;

        Ok(())
    }

    pub fn get_current_version() -> &'static str {
        CURRENT_VERSION
    }
}
