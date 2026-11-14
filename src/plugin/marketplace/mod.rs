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
    /// The UI asked for the install to stop. The worker polls for this between
    /// download chunks and archive entries, undoes any partial extraction, and
    /// leaves the slot in this state.
    Cancelled,
}

pub type InstallSlot = Arc<Mutex<PluginInstallProgress>>;

/// Ask an in-flight install to stop.
///
/// Dropping the [`InstallSlot`] is not enough on its own: the worker thread owns
/// its own clone, so it would keep downloading and keep writing files into the
/// plugin directory. Callers should signal here *and* forget the slot.
pub fn cancel_install(slot: &InstallSlot) {
    if let Ok(mut guard) = slot.lock() {
        // Only a running install can be cancelled — don't clobber a terminal
        // state, or a completed install would report as cancelled.
        if matches!(*guard, PluginInstallProgress::Downloading(_)) {
            *guard = PluginInstallProgress::Cancelled;
        }
    }
}

/// Whether [`cancel_install`] has been called for this slot.
fn is_cancelled(slot: &InstallSlot) -> bool {
    slot.lock()
        .is_ok_and(|g| matches!(*g, PluginInstallProgress::Cancelled))
}

/// How an install worker finished — reported so the caller can tell "stopped
/// early" from "ran all the way through", which a bare `Ok(())` cannot.
enum InstallOutcome {
    /// Extraction finished; the plugin is on disk.
    Completed,
    /// A cancel was observed at a checkpoint and any partial tree was removed.
    Stopped,
}

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

/// How long to wait for the connection itself. Short: a host we cannot reach
/// should fail quickly rather than look like a slow download.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// How long a whole transfer may take.
///
/// `reqwest::blocking` defaults this to **30 seconds**, which on a slow link
/// is a size limit wearing a clock's clothing: a download progressing
/// perfectly well is killed at the 30-second mark. A 1.5 KB plugin took 75
/// seconds on a degraded connection and failed for exactly that reason, and
/// the message — "error sending request for url" — said nothing about why.
///
/// Generous rather than absent: the blocking client has no separate read
/// timeout, so disabling this entirely would let a stalled connection hang
/// forever. Ten minutes is far longer than any plugin needs and still bounded.
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// The HTTP client every marketplace fetch uses.
fn http_client() -> reqwest::Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent("thoth-updater")
        // Unreachable host: fail quickly rather than look like a slow download.
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TRANSFER_TIMEOUT)
        .build()
}

/// A network error with its cause, not just its headline.
///
/// `reqwest`'s `Display` is "error sending request for url (…)" whatever went
/// wrong underneath — a timeout, DNS, a refused connection and a TLS failure
/// all read identically, which makes the difference between "you are offline"
/// and "this took too long" invisible to whoever is reading the message.
fn describe(err: &reqwest::Error) -> String {
    let mut parts = vec![err.to_string()];
    let mut source = std::error::Error::source(err);
    while let Some(cause) = source {
        parts.push(cause.to_string());
        source = cause.source();
    }
    if err.is_timeout() {
        parts.push("the connection stalled — check your network and try again".to_string());
    }
    parts.join(": ")
}

impl MarketPlacePlugin {
    fn download_file_from_github() -> Result<String> {
        let client = http_client()?;

        let response = client.get(constants::MANIFEST_URL).send().map_err(|err| {
            ThothError::DownloadError {
                url: constants::MANIFEST_URL.to_string(),
                reason: format!(
                    "Unable to download manifest file from github - {}",
                    describe(&err)
                ),
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
        // A plugin need not have an icon. Asking for one anyway downloaded
        // nothing into a file that then looked like a cached icon.
        if self.icon_url.trim().is_empty() {
            return Err(ThothError::DownloadError {
                url: String::new(),
                reason: format!("{} declares no icon", self.id),
            });
        }

        let path = PersistentState::plugin_icon_file(&self.id)?;
        let one_week = Duration::from_secs(7 * 24 * 60 * 60);

        let meta = fs::metadata(&path).ok();
        // An empty file is not a cached icon, it is the wreckage of a download
        // that failed — so it must not suppress the next attempt for a week.
        let cached = meta.as_ref().is_some_and(|m| m.len() > 0);
        let fresh = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|elapsed| elapsed <= one_week);

        if !(cached && fresh) {
            let path_clone = path.clone();
            let icon_url = self.icon_url.clone();
            let id = self.id.clone();
            thread::spawn(move || {
                // Fetched whole before anything is written. Creating the file
                // first meant a failure — a dead URL, an empty one, a 404 page
                // — left a zero-byte file behind that every later frame tried
                // to decode and every later open treated as already cached.
                let client = match http_client() {
                    Ok(client) => client,
                    Err(e) => {
                        eprintln!("warn: could not build an http client for {id}: {e}");
                        return;
                    }
                };
                let bytes = match client.get(&icon_url).send() {
                    Err(e) => {
                        eprintln!(
                            "warn: failed to download icon for {id} from {icon_url}: {}",
                            describe(&e)
                        );
                        return;
                    }
                    Ok(response) => {
                        if !response.status().is_success() {
                            eprintln!(
                                "warn: icon for {id} at {icon_url} returned {}",
                                response.status()
                            );
                            return;
                        }
                        match response.bytes() {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                eprintln!("warn: failed to read icon for {id}: {e}");
                                return;
                            }
                        }
                    }
                };
                if bytes.is_empty() {
                    eprintln!("warn: icon for {id} at {icon_url} was empty");
                    return;
                }
                if let Err(e) = fs::write(&path_clone, &bytes) {
                    eprintln!(
                        "warn: failed to write icon to {}: {e}",
                        path_clone.display()
                    );
                    return;
                }
                ctx.request_repaint();
            });
        }

        Ok(path)
    }

    pub fn refresh_icons() -> Result<()> {
        PersistentState::clear_plugins_icon()
    }

    pub fn fetch_readme(url: &str) -> Result<String> {
        let mut response = http_client()?.get(url).send()?;
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
            if let Ok(mut guard) = slot_clone.lock() {
                match result {
                    // Extraction finished, so the plugin *is* installed — report
                    // that even if a cancel landed after the last checkpoint.
                    // Anything else would leave the row saying "not installed"
                    // with the files on disk.
                    Ok(InstallOutcome::Completed) => {
                        *guard = PluginInstallProgress::Complete;
                    }
                    // Stopped at a checkpoint: the partial tree is already gone,
                    // and the slot is left in its `Cancelled` state.
                    Ok(InstallOutcome::Stopped) => {}
                    Err(e) => *guard = PluginInstallProgress::Failed(e.to_string()),
                }
            }
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
    ) -> Result<InstallOutcome> {
        use sha2::{Digest, Sha256};

        let client = http_client()?;

        let mut response = client
            .get(url)
            .send()
            .map_err(|e| ThothError::PluginDownloadError {
                name: plugin_id.to_string(),
                url: url.to_string(),
                reason: describe(&e),
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
            // Checked per chunk so Cancel stops a large download promptly rather
            // than at the next phase boundary. Nothing has been written to disk
            // yet at this point, so there's nothing to undo.
            if is_cancelled(slot) {
                return Ok(InstallOutcome::Stopped);
            }
            let n = response.read(&mut buf)?;
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
            if total_size > 0 {
                let pct = ((data.len() as f64 / total_size as f64) * 85.0) as u8;
                // Don't resurrect a `Downloading` state over a `Cancelled` one
                // set between the check above and here.
                if let Ok(mut guard) = slot.lock()
                    && matches!(*guard, PluginInstallProgress::Downloading(_))
                {
                    *guard = PluginInstallProgress::Downloading(pct);
                }
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
            // From here on the worker is writing into `dest`, so a cancellation
            // has to take the partial tree with it — a half-extracted plugin
            // directory would be picked up as installed on the next launch.
            if is_cancelled(slot) {
                let _ = fs::remove_dir_all(&dest);
                return Ok(InstallOutcome::Stopped);
            }
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
            // As in the download loop: never write progress over a `Cancelled`
            // state set since the checkpoint at the top of this iteration.
            if let Ok(mut guard) = slot.lock()
                && matches!(*guard, PluginInstallProgress::Downloading(_))
            {
                *guard = PluginInstallProgress::Downloading(pct);
            }
            ctx.request_repaint();
        }

        Ok(InstallOutcome::Completed)
    }
}
