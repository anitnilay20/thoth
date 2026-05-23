mod constants;

use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::{
    app::persistent_state::PersistentState,
    error::{Result, ThothError},
};

#[derive(Clone, Debug)]
pub enum PluginInstallProgress {
    Downloading(u8), // 0–99
    Complete,
    Failed(String),
}

pub type InstallSlot = Arc<Mutex<PluginInstallProgress>>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MarketPlacePlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub download_url: String,
    pub sha256: String,
    pub icon_url: String,
    pub repo_url: String,
    pub readme: String,
    #[serde(default)]
    pub categories: Vec<String>,
}

pub type ManifestData = HashMap<String, MarketPlacePlugin>;

impl MarketPlacePlugin {
    fn download_file_from_github() -> Result<String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("thoth-updater")
            .build()?;

        let response = client.get(constants::MANIFEST_URL).send().map_err(|err| {
            ThothError::DownloadError {
                url: constants::MANIFEST_URL.to_string(),
                reason: format!("Unable to download manifest file from github - {}", err),
            }
        })?;

        response.text().map_err(|err| ThothError::DownloadError {
            url: constants::MANIFEST_URL.to_string(),
            reason: format!("Unable to get file contents - {}", err),
        })
    }

    fn update_local_file(path: PathBuf) -> Result<()> {
        let contents = Self::download_file_from_github()?;
        fs::write(path, contents)?;

        Ok(())
    }

    fn read_data_from_file(path: PathBuf) -> Result<ManifestData> {
        let content = fs::read_to_string(path).map_err(|err| ThothError::DownloadError {
            url: "".to_string(),
            reason: format!("Unable to read from local manifest file - {}", err),
        })?;

        toml::from_str::<ManifestData>(&content).map_err(|err| ThothError::DownloadError {
            url: "".to_string(),
            reason: format!("Unable to parse manifest data - {}", err),
        })
    }

    pub fn get_manifest_data(force: bool) -> Result<ManifestData> {
        let one_day = Duration::from_secs(24 * 60 * 60);
        let path = PersistentState::marketplace_registry_file()?;

        if !path.exists() || force {
            Self::update_local_file(path.clone())?;
        }

        if let Ok(metadata) = fs::metadata(path.clone())
            && let Ok(modified) = metadata.modified()
            && let Ok(elapsed) = modified.elapsed()
            && elapsed > one_day
        {
            let path_clone = path.clone();
            thread::spawn(|| {
                let _ = Self::update_local_file(path_clone)
                    .map_err(|err| eprintln!("Unable to update local file - {}", err));
            });
        }

        Self::read_data_from_file(path)
    }

    pub fn get_icon_file(&self, ctx: egui::Context) -> Result<PathBuf> {
        let path = PersistentState::plugin_icon_file(&self.id)?;
        let one_week = Duration::from_secs(7 * 24 * 60 * 60);

        let needs_download = !path.exists()
            || fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.elapsed().ok())
                .map(|elapsed| elapsed > one_week)
                .unwrap_or(true);

        if needs_download {
            let path_clone = path.clone();
            let icon_url = self.icon_url.clone();
            thread::spawn(move || match File::create(&path_clone) {
                Err(e) => eprintln!(
                    "warn: failed to create icon file {}: {e}",
                    path_clone.display()
                ),
                Ok(mut file) => match reqwest::blocking::get(&icon_url) {
                    Err(e) => eprintln!("warn: failed to download icon from {icon_url}: {e}"),
                    Ok(mut response) => {
                        if let Err(e) = std::io::copy(&mut response, &mut file) {
                            eprintln!(
                                "warn: failed to write icon to {}: {e}",
                                path_clone.display()
                            );
                        }
                        ctx.request_repaint();
                    }
                },
            });
        }

        Ok(path)
    }

    pub fn refresh_icons() -> Result<()> {
        PersistentState::clear_plugins_icon()
    }

    pub fn fetch_readme(url: &str) -> Result<String> {
        let mut response = reqwest::blocking::get(url)?;
        let mut readme = String::new();
        response.read_to_string(&mut readme)?;
        Ok(readme)
    }

    /// Spawns a background thread that downloads and installs the plugin.
    /// Returns an `InstallSlot` that the UI can poll each frame for progress.
    pub fn download_and_install(&self, ctx: egui::Context) -> InstallSlot {
        let slot: InstallSlot = Arc::new(Mutex::new(PluginInstallProgress::Downloading(0)));
        let slot_clone = slot.clone();
        let url = self.download_url.clone();
        let sha256 = self.sha256.clone();
        let id = self.id.clone();

        thread::spawn(move || {
            let result = Self::perform_install(&url, &sha256, &id, &slot_clone, &ctx);
            *slot_clone.lock().unwrap() = match result {
                Ok(()) => PluginInstallProgress::Complete,
                Err(e) => PluginInstallProgress::Failed(e.to_string()),
            };
            ctx.request_repaint();
        });

        slot
    }

    fn perform_install(
        url: &str,
        expected_sha256: &str,
        plugin_id: &str,
        slot: &InstallSlot,
        ctx: &egui::Context,
    ) -> Result<()> {
        use sha2::{Digest, Sha256};

        let client = reqwest::blocking::Client::builder()
            .user_agent("thoth-updater")
            .build()?;

        let mut response = client
            .get(url)
            .send()
            .map_err(|e| ThothError::PluginDownloadError {
                name: plugin_id.to_string(),
                url: url.to_string(),
                reason: e.to_string(),
            })?;

        if !response.status().is_success() {
            return Err(ThothError::PluginDownloadError {
                name: plugin_id.to_string(),
                url: url.to_string(),
                reason: format!("HTTP {}", response.status()),
            });
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut data: Vec<u8> = Vec::new();
        let mut buf = vec![0u8; 8192];

        loop {
            let n = response.read(&mut buf)?;
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
            if total_size > 0 {
                let pct = ((data.len() as f64 / total_size as f64) * 85.0) as u8;
                *slot.lock().unwrap() = PluginInstallProgress::Downloading(pct);
                ctx.request_repaint();
            }
        }

        let hash = Sha256::digest(&data);
        let hex = format!("{hash:x}");
        if expected_sha256.is_empty() {
            return Err(ThothError::PluginDownloadError {
                name: plugin_id.to_string(),
                url: url.to_string(),
                reason: "SHA256 checksum is missing from the plugin manifest".to_string(),
            });
        }
        if hex != expected_sha256 {
            return Err(ThothError::PluginDownloadError {
                name: plugin_id.to_string(),
                url: url.to_string(),
                reason: format!("SHA256 mismatch: expected {expected_sha256}, got {hex}"),
            });
        }

        *slot.lock().unwrap() = PluginInstallProgress::Downloading(90);
        ctx.request_repaint();

        let cursor = std::io::Cursor::new(data);
        let mut archive =
            zip::ZipArchive::new(cursor).map_err(|e| ThothError::PluginDownloadError {
                name: plugin_id.to_string(),
                url: url.to_string(),
                reason: format!("Invalid zip archive: {e}"),
            })?;

        let dest = PersistentState::plugin_install_dir_by_id(plugin_id)?;
        let len = archive.len();

        // Detect if all entries share a single top-level directory (common in GitHub release zips).
        // If so, strip that prefix so files land directly in `dest/` not `dest/{wrapper}/`.
        let strip_prefix: Option<String> = {
            let mut prefix: Option<String> = None;
            let mut consistent = true;
            for i in 0..len {
                if let Ok(entry) = archive.by_index(i) {
                    let name = entry.name().to_string();
                    let trimmed = name.trim_start_matches('/');
                    if trimmed.is_empty() {
                        continue;
                    }
                    let first = trimmed.split('/').next().unwrap_or("").to_string();
                    if first.is_empty() {
                        continue;
                    }
                    match &prefix {
                        None => prefix = Some(first),
                        Some(p) if *p == first => {}
                        _ => {
                            consistent = false;
                            break;
                        }
                    }
                }
            }
            if consistent { prefix } else { None }
        };

        for i in 0..len {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| ThothError::PluginDownloadError {
                    name: plugin_id.to_string(),
                    url: url.to_string(),
                    reason: format!("Failed to read zip entry {i}: {e}"),
                })?;

            let raw_name = entry.name().to_string();
            let trimmed = raw_name.trim_start_matches('/');

            // Strip the common top-level wrapper directory if present
            let rel_path = if let Some(ref pfx) = strip_prefix {
                trimmed.strip_prefix(&format!("{pfx}/")).unwrap_or(trimmed)
            } else {
                trimmed
            };

            if rel_path.is_empty() {
                continue;
            }

            let out_path = dest.join(rel_path);
            // Zip-slip guard: normalize away ".." components and verify the
            // resolved path is still inside dest. We can't use canonicalize()
            // on paths that don't exist yet, so resolve via components instead.
            let out_path = {
                use std::path::Component;
                let mut resolved = std::path::PathBuf::new();
                for c in out_path.components() {
                    match c {
                        Component::ParentDir => {
                            resolved.pop();
                        }
                        Component::CurDir => {}
                        _ => resolved.push(c),
                    }
                }
                resolved
            };
            if !out_path.starts_with(&dest) {
                continue;
            }

            if entry.is_dir() {
                fs::create_dir_all(&out_path)?;
            } else {
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut out_file =
                    File::create(&out_path).map_err(|e| ThothError::FileSaveError {
                        path: out_path.clone(),
                        reason: e.to_string(),
                    })?;
                std::io::copy(&mut entry, &mut out_file)?;
            }

            let pct = (90 + ((i + 1) as f64 / len as f64 * 9.0) as u8).min(99);
            *slot.lock().unwrap() = PluginInstallProgress::Downloading(pct);
            ctx.request_repaint();
        }

        Ok(())
    }
}
