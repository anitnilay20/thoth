use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub published_at: String,
    pub html_url: String,
    pub prerelease: bool,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpdateAvailable {
        latest_version: String,
        current_version: String,
        releases: Vec<ReleaseInfo>,
    },
    Downloading {
        progress: f32,
        version: String,
    },
    ReadyToInstall {
        version: String,
        path: std::path::PathBuf,
    },
    Installing,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct UpdateStatus {
    pub state: UpdateState,
    pub last_check: Option<DateTime<Utc>>,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            state: UpdateState::Idle,
            last_check: None,
        }
    }
}

impl UpdateStatus {
    /// Check if we should check for updates based on settings
    pub fn should_check(&self, check_interval_hours: u64, auto_check: bool) -> bool {
        // Don't check if auto-check is disabled
        if !auto_check {
            return false;
        }

        match self.last_check {
            None => true, // Never checked before
            Some(last_check) => {
                let now = Utc::now();
                let duration = now.signed_duration_since(last_check);
                duration.num_hours() >= check_interval_hours as i64
            }
        }
    }
}
