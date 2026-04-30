use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
};

use eframe::egui;

use crate::plugin::{
    manager::PluginManager,
    marketplace::{InstallSlot, ManifestData, MarketPlacePlugin, PluginInstallProgress},
};

pub enum DetailAction {
    Install,
    Uninstall,
    Enable,
    Disable,
    Retry,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum SortOrder {
    #[default]
    NameAZ,
    NameZA,
}

pub(super) fn category_glyph(cap: &str) -> &'static str {
    match cap.to_ascii_lowercase().as_str() {
        "file-loader" | "fileloader" => egui_phosphor::regular::FILE_PLUS,
        "file-viewer" | "fileviewer" => egui_phosphor::regular::FILE_TEXT,
        "data-source" | "datasource" => egui_phosphor::regular::DATABASE,
        "exporter" => egui_phosphor::regular::ARROW_UP,
        "search-provider" | "searchprovider" => egui_phosphor::regular::MAGNIFYING_GLASS,
        "new-ui-component" | "newuicomponent" => egui_phosphor::regular::SQUARES_FOUR,
        "theme" => egui_phosphor::regular::SLIDERS,
        _ => egui_phosphor::regular::PUZZLE_PIECE,
    }
}

pub(super) fn category_label(cap: &str) -> &'static str {
    match cap.to_ascii_lowercase().as_str() {
        "file-loader" | "fileloader" => "File Loader",
        "file-viewer" | "fileviewer" => "File Viewer",
        "data-source" | "datasource" => "Data Source",
        "exporter" => "Exporter",
        "search-provider" | "searchprovider" => "Search Provider",
        "new-ui-component" | "newuicomponent" => "New UI Component",
        "theme" => "Theme",
        _ => "Plugin",
    }
}

type Pending = Option<Arc<Mutex<Option<Result<String, String>>>>>;

#[derive(Clone, Default)]
pub struct ReadmeCacheEntry {
    pub content: Option<String>,
    pub error: Option<String>,
    pub pending: Pending,
}

impl ReadmeCacheEntry {
    pub fn readme_key(plugin_id: &str) -> egui::Id {
        egui::Id::new(("mp_readme", plugin_id))
    }

    pub fn load(ctx: &egui::Context, plugin: &MarketPlacePlugin) -> Self {
        let key = Self::readme_key(&plugin.id);
        ctx.data_mut(|d| d.get_temp::<Self>(key).unwrap_or_default())
    }

    pub fn save(&self, ctx: &egui::Context, plugin_id: &str) {
        let key = Self::readme_key(plugin_id);
        ctx.data_mut(|d| d.insert_temp(key, self.clone()));
    }

    pub fn needs_fetch(&self) -> bool {
        self.content.is_none() && self.error.is_none() && self.pending.is_none()
    }

    pub fn start_fetch(&mut self, ctx: &egui::Context, plugin: &MarketPlacePlugin) {
        let slot: Arc<Mutex<Option<Result<String, String>>>> = Arc::new(Mutex::new(None));
        let slot_clone = slot.clone();
        let ctx_clone = ctx.clone();
        let readme_url = plugin.readme.clone();
        thread::spawn(move || {
            let result = MarketPlacePlugin::fetch_readme(&readme_url).map_err(|e| e.to_string());
            *slot_clone.lock().unwrap() = Some(result);
            ctx_clone.request_repaint();
        });
        self.pending = Some(slot);
    }

    pub fn poll(&mut self) {
        let result = self
            .pending
            .as_ref()
            .and_then(|slot| slot.lock().ok()?.take());
        match result {
            None => {}
            Some(Ok(text)) => {
                self.content = Some(text);
                self.pending = None;
            }
            Some(Err(e)) => {
                self.error = Some(e);
                self.pending = None;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum InstallState {
    #[default]
    NotInstalled,
    Installing(u8), // 0-100
    Installed,
    Update,
    Disabled,
    Failed(String),
}

type PendingManifest = Arc<Mutex<Option<Result<ManifestData, String>>>>;

#[derive(Clone)]
pub struct MarketplaceUiState {
    pub search_query: String,
    pub selected_id: Option<String>,
    /// "all" | "installed" | "updates"
    pub selected_category: String,
    pub plugins: Vec<MarketPlacePlugin>,
    pub install_states: HashMap<String, InstallState>,
    /// Active download/install threads keyed by plugin id.
    pub install_handles: HashMap<String, InstallSlot>,
    pub load_error: Option<String>,
    pub loaded: bool,
    /// True while the background manifest fetch is in flight.
    pub loading: bool,
    /// Slot written by the background fetch thread; polled each frame.
    pub pending: Option<PendingManifest>,
    pub sort: SortOrder,
}

impl Default for MarketplaceUiState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            selected_id: None,
            selected_category: "all".to_string(),
            plugins: Vec::new(),
            install_states: HashMap::new(),
            install_handles: HashMap::new(),
            load_error: None,
            loaded: false,
            loading: false,
            pending: None,
            sort: SortOrder::default(),
        }
    }
}

impl MarketplaceUiState {
    /// Kick off the background manifest fetch if not already loaded/loading.
    pub fn load_if_needed(&mut self, ctx: &egui::Context, force: bool) {
        if (self.loaded || self.loading) && !force {
            return;
        }
        self.loading = true;
        self.loaded = false;
        self.load_error = None;

        if force {
            let _ = MarketPlacePlugin::refresh_icons();
        }

        let slot: PendingManifest = Arc::new(Mutex::new(None));
        let slot_clone = slot.clone();
        let ctx_clone = ctx.clone();

        thread::spawn(move || {
            let result = MarketPlacePlugin::get_manifest_data(force).map_err(|e| e.to_string());
            *slot_clone.lock().unwrap() = Some(result);
            ctx_clone.request_repaint();
        });

        self.pending = Some(slot);
    }

    /// Poll all active install threads and update `install_states`.
    /// Returns ids of installs that just completed (ok or failed).
    pub fn poll_installs(&mut self) -> Vec<(String, Result<(), String>)> {
        let mut state_updates: Vec<(String, InstallState)> = Vec::new();
        let mut completed: Vec<(String, Result<(), String>)> = Vec::new();

        for (id, slot) in &self.install_handles {
            let progress = slot.lock().ok().map(|g| g.clone());
            match progress {
                Some(PluginInstallProgress::Downloading(pct)) => {
                    state_updates.push((id.clone(), InstallState::Installing(pct)));
                }
                Some(PluginInstallProgress::Complete) => {
                    state_updates.push((id.clone(), InstallState::Installed));
                    completed.push((id.clone(), Ok(())));
                }
                Some(PluginInstallProgress::Failed(e)) => {
                    state_updates.push((id.clone(), InstallState::Failed(e.clone())));
                    completed.push((id.clone(), Err(e)));
                }
                None => {}
            }
        }

        for (id, state) in state_updates {
            self.install_states.insert(id, state);
        }
        for (id, _) in &completed {
            self.install_handles.remove(id);
        }

        completed
    }

    /// Check whether the background fetch has completed and apply the result.
    /// Must be called every frame.
    pub fn poll_pending(&mut self, disabled_plugins: &[String]) {
        let result = self
            .pending
            .as_ref()
            .and_then(|slot| slot.lock().ok()?.take());

        match result {
            None => {} // still in flight or no pending fetch
            Some(Ok(data)) => {
                let mut install_states = HashMap::new();
                // let disabled_plugins = settings::Settings::read(ctx);

                PluginManager::get_installed_plugn()
                    .iter()
                    .for_each(|(key, i_p)| {
                        if let Some(m_p) = data.get(key) {
                            if disabled_plugins.contains(&i_p.id) {
                                install_states.insert(key.clone(), InstallState::Disabled);
                            } else if i_p.version != m_p.version {
                                install_states.insert(key.clone(), InstallState::Update);
                            } else {
                                install_states.insert(key.clone(), InstallState::Installed);
                            }
                        }
                    });

                let mut plugins: Vec<MarketPlacePlugin> = data.into_values().collect();
                plugins.sort_by(|a, b| a.name.cmp(&b.name));

                self.plugins = plugins;
                self.install_states = install_states;
                self.loading = false;
                self.loaded = true;
                self.pending = None;
            }
            Some(Err(e)) => {
                self.load_error = Some(e);
                self.loading = false;
                self.loaded = true;
                self.pending = None;
            }
        }
    }
}
