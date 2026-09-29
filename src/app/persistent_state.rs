use crate::error::{Result, ThothError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::constants::{DEFAULT_SIDEBAR_WIDTH, MAX_RECENT_FILES, MIN_SIDEBAR_WIDTH};

const MAX_SAVED_QUERIES: usize = 200; // Maximum number of saved queries

/// What kind of content a persisted tab holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PersistedTabKind {
    File {
        path: String,
    },
    Plugin {
        plugin_id: String,
        /// Per-tab state blob from the plugin's `get-state` hook, passed back via
        /// `init-with-state` on restore. Absent for older sessions / plugins
        /// without a `tab-host` export.
        #[serde(default)]
        state: Option<String>,
    },
    /// A Chart Studio chart, stored as an opaque JSON snapshot (columns + rows
    /// + spec) so it re-renders without needing its original data source.
    Chart {
        state: String,
    },
}

/// A tab entry that can be restored on the next launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedTab {
    pub kind: PersistedTabKind,
}

/// A bookmark for a specific JSON path within a file
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedQuery {
    /// Stable id, used to apply, update or delete it.
    pub id: String,
    /// What the user called it, or a description of the query itself when
    /// they saved it without naming it.
    pub name: String,
    /// The file it was written against.
    ///
    /// A query names columns, so it only fits the file it was built on —
    /// offering it elsewhere would be offering something that fails on use.
    pub file_path: String,
    /// The lanes.
    #[serde(default)]
    pub spec: thoth_plugin_sdk::components::QuerySpec,
    /// Typed SQL, when the query was written rather than built. Saved
    /// alongside the lanes rather than instead of them, so reverting to the
    /// lanes still lands on the query the SQL grew out of.
    #[serde(default)]
    pub sql: Option<String>,
    /// When it was saved.
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentState {
    #[serde(default)]
    recent_files: Vec<String>,
    #[serde(default = "default_sidebar_width")]
    sidebar_width: f32,
    #[serde(default)]
    sidebar_expanded: bool,
    #[serde(default)]
    saved_queries: Vec<SavedQuery>,
    /// Tabs open at last save — restored on next launch.
    #[serde(default)]
    open_tabs: Vec<PersistedTab>,
    /// Index into `open_tabs` of the tab that was active at last save.
    #[serde(default)]
    active_tab_index: usize,
}

fn default_sidebar_width() -> f32 {
    DEFAULT_SIDEBAR_WIDTH
}

impl Default for PersistentState {
    /// A blank state — **no disk access**.
    ///
    /// `Default` used to call [`load`](PersistentState::load), which made
    /// constructing one implicitly read the user's real saved session. That is
    /// surprising for a `Default`, and it leaked the developer's own open tabs and
    /// recent files into tests. Call [`load_or_default`](PersistentState::load_or_default)
    /// when you actually want the persisted state.
    fn default() -> Self {
        Self::blank()
    }
}

impl PersistentState {
    /// An empty in-memory state, touching no files. Use this in tests so they
    /// don't inherit whatever session the developer happens to have open.
    pub fn blank() -> Self {
        Self {
            recent_files: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            sidebar_expanded: false,
            saved_queries: Vec::new(),
            open_tabs: Vec::new(),
            active_tab_index: 0,
        }
    }

    /// The persisted state from disk, or a [`blank`](PersistentState::blank) one if
    /// it can't be read. This is what the running app wants at startup.
    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_else(|err| {
            eprintln!("Failed to load persistent state: {}", err);
            Self::blank()
        })
    }

    /// Get the path to the app state storage
    /// Returns: ~/.config/thoth/persistent_state.json on Linux/macOS
    ///          %APPDATA%/thoth/persistent_state.json on Windows
    fn storage_path() -> Result<PathBuf> {
        let thoth_config_dir =
            crate::config_dir::config_root().ok_or_else(|| ThothError::StateError {
                reason: "Failed to get config directory".to_string(),
            })?;

        // Create directory if it doesn't exist
        if !thoth_config_dir.exists() {
            std::fs::create_dir_all(&thoth_config_dir).map_err(|e| ThothError::StateError {
                reason: format!("Failed to create thoth config directory: {}", e),
            })?;
        }

        Ok(thoth_config_dir.join("persistent_state.json"))
    }

    /// Load app state from disk
    pub fn load() -> Result<Self> {
        let path = Self::storage_path()?;

        if path.exists() {
            let contents = std::fs::read_to_string(&path).map_err(|e| ThothError::StateError {
                reason: format!("Failed to read app state: {}", e),
            })?;
            let app_state: PersistentState =
                serde_json::from_str(&contents).map_err(|e| ThothError::StateError {
                    reason: format!("Failed to parse app state: {}", e),
                })?;
            Ok(app_state)
        } else {
            // Try to migrate from old recent_files.json
            Self::migrate_from_old_format()
        }
    }

    /// Migrate from old recent_files.json format
    fn migrate_from_old_format() -> Result<Self> {
        let old_path = crate::config_dir::config_root()
            .ok_or_else(|| ThothError::StateError {
                reason: "Failed to get config directory".to_string(),
            })?
            .join("recent_files.json");

        if old_path.exists() {
            // Read old format
            let contents =
                std::fs::read_to_string(&old_path).map_err(|e| ThothError::StateError {
                    reason: format!("Failed to read old recent files: {}", e),
                })?;

            #[derive(Deserialize)]
            struct OldFormat {
                files: Vec<String>,
            }

            if let Ok(old_data) = serde_json::from_str::<OldFormat>(&contents) {
                eprintln!("Migrating from old recent_files.json format...");
                let new_state = PersistentState {
                    recent_files: old_data.files,
                    ..Self::blank()
                };

                // Save in new format
                if new_state.save().is_ok() {
                    // Remove old file
                    let _ = std::fs::remove_file(&old_path);
                    eprintln!("Migration successful!");
                }

                return Ok(new_state);
            }
        }

        // No migration needed or failed, return default
        Ok(Self::blank())
    }

    /// Save app state to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::storage_path()?;
        let json = serde_json::to_string_pretty(self).map_err(|e| ThothError::StateError {
            reason: format!("Failed to serialize app state: {}", e),
        })?;
        std::fs::write(&path, &json).map_err(|e| ThothError::FileWriteError {
            path: path.clone(),
            reason: e.to_string(),
        })?;
        Ok(())
    }

    // Recent Files methods

    /// Add a file to recent files (moves to top if already exists)
    pub fn add_recent_file(&mut self, file_path: String, max_recent_files: usize) {
        // Remove if already exists
        self.recent_files.retain(|f| f != &file_path);

        // Add to front
        self.recent_files.insert(0, file_path);

        // Limit to configured max (or fallback to constant)
        let limit = if max_recent_files > 0 {
            max_recent_files
        } else {
            MAX_RECENT_FILES
        };

        if self.recent_files.len() > limit {
            self.recent_files.truncate(limit);
        }
    }

    /// Remove a file from recent files
    pub fn remove_recent_file(&mut self, file_path: &str) {
        self.recent_files.retain(|f| f != file_path);
    }

    /// Get all recent files
    pub fn get_recent_files(&self) -> &[String] {
        &self.recent_files
    }

    // Sidebar width methods

    /// Set the sidebar width
    pub fn set_sidebar_width(&mut self, width: f32) {
        self.sidebar_width = width.max(MIN_SIDEBAR_WIDTH); // Ensure minimum width
    }

    /// Get the sidebar width
    pub fn get_sidebar_width(&self) -> f32 {
        self.sidebar_width
    }

    // Sidebar expanded state methods

    /// Set sidebar expanded state
    pub fn set_sidebar_expanded(&mut self, expanded: bool) {
        self.sidebar_expanded = expanded;
    }

    /// Get sidebar expanded state
    pub fn get_sidebar_expanded(&self) -> bool {
        self.sidebar_expanded
    }

    // Saved queries

    /// Save the current query against `file_path`, returning its id.
    ///
    /// Named by the user, or described from the query itself when they saved
    /// it without typing a name — an unnamed row in a list is not something
    /// anyone picks from later.
    pub fn save_query(
        &mut self,
        file_path: String,
        name: String,
        spec: thoth_plugin_sdk::components::QuerySpec,
        sql: Option<String>,
    ) -> String {
        let name = match name.trim() {
            "" => spec.summary(),
            named => named.to_string(),
        };
        // Unique against what is already stored, not merely against the
        // clock: two queries saved in the same millisecond would otherwise
        // share an id, and deleting one would delete both.
        let stamp = Self::current_timestamp_millis();
        let mut id = format!("q{stamp}");
        let mut nth = 1;
        while self.saved_queries.iter().any(|q| q.id == id) {
            id = format!("q{stamp}-{nth}");
            nth += 1;
        }
        self.saved_queries.insert(
            0,
            SavedQuery {
                id: id.clone(),
                name,
                file_path,
                spec,
                sql,
                created_at: Self::current_timestamp(),
            },
        );
        if self.saved_queries.len() > MAX_SAVED_QUERIES {
            self.saved_queries.truncate(MAX_SAVED_QUERIES);
        }
        id
    }

    /// Point an existing saved query at the query as it now stands.
    pub fn update_query(
        &mut self,
        id: &str,
        spec: thoth_plugin_sdk::components::QuerySpec,
        sql: Option<String>,
    ) {
        if let Some(saved) = self.saved_queries.iter_mut().find(|q| q.id == id) {
            saved.spec = spec;
            saved.sql = sql;
        }
    }

    /// Forget a saved query.
    pub fn remove_query(&mut self, id: &str) {
        self.saved_queries.retain(|q| q.id != id);
    }

    /// The queries saved against `file_path`, newest first.
    pub fn saved_queries(&self, file_path: &str) -> Vec<&SavedQuery> {
        self.saved_queries
            .iter()
            .filter(|q| q.file_path == file_path)
            .collect()
    }

    /// One saved query by id.
    pub fn saved_query(&self, id: &str) -> Option<&SavedQuery> {
        self.saved_queries.iter().find(|q| q.id == id)
    }

    // Tab session methods

    /// Replace the full list of persisted tabs and which one was active.
    pub fn set_open_tabs(&mut self, tabs: Vec<PersistedTab>, active_index: usize) {
        self.active_tab_index = if tabs.is_empty() {
            0
        } else {
            active_index.min(tabs.len() - 1)
        };
        self.open_tabs = tabs;
    }

    /// Return the tabs saved from the previous session.
    pub fn get_open_tabs(&self) -> &[PersistedTab] {
        &self.open_tabs
    }

    /// Return the index of the tab that was active at last save.
    pub fn get_active_tab_index(&self) -> usize {
        self.active_tab_index
    }

    /// Milliseconds since the Unix epoch, so two queries saved in the same
    /// second still get distinct ids.
    fn current_timestamp_millis() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    }

    /// Seconds since the Unix epoch, for stamping a saved query.
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    pub fn local_plugin_dir(plugin_id: &str) -> Result<PathBuf> {
        let dir = crate::config_dir::config_root()
            .ok_or_else(|| "failed to locate config directory".to_string())?
            .join("data")
            .join("plugins")
            .join(plugin_id);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        Ok(dir)
    }

    pub fn plugin_state_path(plugin_id: &str) -> Result<PathBuf> {
        let dir = Self::local_plugin_dir(plugin_id)?;
        Ok(dir.join("state.json"))
    }

    pub fn marketplace_dir() -> Result<PathBuf> {
        let dir = crate::config_dir::config_root()
            .ok_or_else(|| "failed to locate config directory".to_string())?
            .join("marketplace");

        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        Ok(dir)
    }

    pub fn plugin_install_dir() -> Result<PathBuf> {
        let mp_dir = Self::marketplace_dir()?;
        let dir = mp_dir.join("installs");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(dir)
    }

    pub fn plugin_install_dir_by_id(plugin_id: &str) -> Result<PathBuf> {
        let dir = Self::plugin_install_dir()?.join(plugin_id);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(dir)
    }

    pub fn marketplace_registry_file() -> Result<PathBuf> {
        let dir = Self::marketplace_dir()?;
        Ok(dir.join("manifest.toml"))
    }

    pub fn plugin_icon_dir() -> Result<PathBuf> {
        let dir = Self::marketplace_dir()?.join("icons");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(dir)
    }

    pub fn plugin_icon_file(plugin_id: &str) -> Result<PathBuf> {
        let dir = Self::plugin_icon_dir()?;
        Ok(dir.join(plugin_id))
    }

    pub fn clear_plugins_icon() -> Result<()> {
        let dir = Self::plugin_icon_dir()?;
        match fs::remove_dir_all(dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_recent_file() {
        let mut state = PersistentState {
            recent_files: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            sidebar_expanded: false,
            saved_queries: Vec::new(),
            open_tabs: Vec::new(),
            active_tab_index: 0,
        };
        state.add_recent_file("file1.json".to_string(), MAX_RECENT_FILES);
        state.add_recent_file("file2.json".to_string(), MAX_RECENT_FILES);

        assert_eq!(state.get_recent_files().len(), 2);
        assert_eq!(state.get_recent_files()[0], "file2.json");
        assert_eq!(state.get_recent_files()[1], "file1.json");
    }

    #[test]
    fn test_add_duplicate_moves_to_top() {
        let mut state = PersistentState {
            recent_files: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            sidebar_expanded: false,
            saved_queries: Vec::new(),
            open_tabs: Vec::new(),
            active_tab_index: 0,
        };
        state.add_recent_file("file1.json".to_string(), MAX_RECENT_FILES);
        state.add_recent_file("file2.json".to_string(), MAX_RECENT_FILES);
        state.add_recent_file("file1.json".to_string(), MAX_RECENT_FILES);

        assert_eq!(state.get_recent_files().len(), 2);
        assert_eq!(state.get_recent_files()[0], "file1.json");
        assert_eq!(state.get_recent_files()[1], "file2.json");
    }

    #[test]
    fn test_max_recent_files() {
        let mut state = PersistentState {
            recent_files: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            sidebar_expanded: false,
            saved_queries: Vec::new(),
            open_tabs: Vec::new(),
            active_tab_index: 0,
        };
        for i in 0..15 {
            state.add_recent_file(format!("file{}.json", i), MAX_RECENT_FILES);
        }

        assert_eq!(state.get_recent_files().len(), MAX_RECENT_FILES);
        assert_eq!(state.get_recent_files()[0], "file14.json");
    }

    #[test]
    fn test_remove_recent_file() {
        let mut state = PersistentState {
            recent_files: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            sidebar_expanded: false,
            saved_queries: Vec::new(),
            open_tabs: Vec::new(),
            active_tab_index: 0,
        };
        state.add_recent_file("file1.json".to_string(), MAX_RECENT_FILES);
        state.add_recent_file("file2.json".to_string(), MAX_RECENT_FILES);
        state.remove_recent_file("file1.json");

        assert_eq!(state.get_recent_files().len(), 1);
        assert_eq!(state.get_recent_files()[0], "file2.json");
    }

    #[test]
    fn test_sidebar_width() {
        let mut state = PersistentState {
            recent_files: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            sidebar_expanded: false,
            saved_queries: Vec::new(),
            open_tabs: Vec::new(),
            active_tab_index: 0,
        };

        assert_eq!(state.get_sidebar_width(), DEFAULT_SIDEBAR_WIDTH);

        state.set_sidebar_width(500.0);
        assert_eq!(state.get_sidebar_width(), 500.0);

        // Values below MIN_SIDEBAR_WIDTH are clamped to the minimum
        state.set_sidebar_width(100.0);
        assert_eq!(state.get_sidebar_width(), MIN_SIDEBAR_WIDTH);
    }

    /// A `PersistentState` with nothing in it, for the saved-query tests.
    fn empty_state() -> PersistentState {
        PersistentState {
            recent_files: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            sidebar_expanded: false,
            saved_queries: Vec::new(),
            open_tabs: Vec::new(),
            active_tab_index: 0,
        }
    }

    fn spec_on(field: &str) -> thoth_plugin_sdk::components::QuerySpec {
        use thoth_plugin_sdk::components::{ColumnType, Filter, Operator, QuerySpec};
        QuerySpec {
            filters: vec![Filter {
                field: field.to_string(),
                operator: Operator::Equals,
                values: vec!["x".to_string()],
                column: ColumnType::Text,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn a_saved_query_belongs_to_the_file_it_was_written_on() {
        // A query names columns, so offering it for another file would be
        // offering something that fails the moment it is applied.
        let mut state = empty_state();
        state.save_query("/a.json".into(), "errors".into(), spec_on("level"), None);
        state.save_query("/b.json".into(), "slow".into(), spec_on("ms"), None);

        let a = state.saved_queries("/a.json");
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].name, "errors");
        assert_eq!(state.saved_queries("/b.json").len(), 1);
        assert!(state.saved_queries("/never-opened.json").is_empty());
    }

    #[test]
    fn saving_without_a_name_describes_the_query_instead() {
        // A list of rows called "Untitled" is not one anybody picks from.
        let mut state = empty_state();
        let id = state.save_query("/a.json".into(), "   ".into(), spec_on("level"), None);
        let saved = state.saved_query(&id).expect("just saved");
        assert!(!saved.name.trim().is_empty());
        assert_eq!(saved.name, spec_on("level").summary());
    }

    #[test]
    fn typed_sql_is_saved_alongside_the_lanes_not_instead_of_them() {
        // Reverting to the lanes has to land on the query the SQL grew out of.
        let mut state = empty_state();
        let sql = Some("SELECT 1".to_string());
        let id = state.save_query(
            "/a.json".into(),
            "raw".into(),
            spec_on("level"),
            sql.clone(),
        );
        let saved = state.saved_query(&id).unwrap();
        assert_eq!(saved.sql, sql);
        assert_eq!(saved.spec, spec_on("level"));
    }

    #[test]
    fn updating_points_a_saved_query_at_the_query_as_it_now_stands() {
        let mut state = empty_state();
        let id = state.save_query("/a.json".into(), "q".into(), spec_on("level"), None);
        state.update_query(&id, spec_on("service"), Some("SELECT 2".into()));

        let saved = state.saved_query(&id).unwrap();
        assert_eq!(saved.spec, spec_on("service"));
        assert_eq!(saved.sql.as_deref(), Some("SELECT 2"));
        // Its name and id are its identity and do not move with its contents.
        assert_eq!(saved.name, "q");
    }

    #[test]
    fn removing_one_leaves_the_others() {
        let mut state = empty_state();
        let first = state.save_query("/a.json".into(), "one".into(), spec_on("a"), None);
        let second = state.save_query("/a.json".into(), "two".into(), spec_on("b"), None);

        state.remove_query(&first);
        assert!(state.saved_query(&first).is_none());
        assert!(state.saved_query(&second).is_some());
        assert_eq!(state.saved_queries("/a.json").len(), 1);
    }

    #[test]
    fn the_newest_query_is_offered_first_and_the_list_is_bounded() {
        let mut state = empty_state();
        for i in 0..=MAX_SAVED_QUERIES {
            state.save_query("/a.json".into(), format!("q{i}"), spec_on("level"), None);
        }
        assert_eq!(state.saved_queries.len(), MAX_SAVED_QUERIES);
        // Newest first, so the list reads as a history rather than as whatever
        // order the file happened to be written in.
        assert_eq!(
            state.saved_queries("/a.json")[0].name,
            format!("q{MAX_SAVED_QUERIES}")
        );
    }
}
