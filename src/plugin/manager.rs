use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use wasmtime::component::ResourceTable;
use wasmtime::{Cache, CacheConfig, Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView};

use crate::PLUGIN_MANAGER;
use crate::app::persistent_state::PersistentState;
use crate::error::Result;
use crate::notification::{Notification, NotificationManager, NotificationStatus};
use crate::plugin::Capability;
use crate::plugin::network_policy::NetworkPolicy;
use crate::plugin::plugin_registry::PluginRegistry;
use crate::plugin::wasm_data_source::WasmDataSourceLoader;
use crate::plugin::wasm_file_viewer_loader::WasmFileViewerLoader;
use crate::plugin::wasm_loader::WasmFileLoader;
use crate::plugin::wasm_plugin_settings::WasmPluginSettings;
use crate::settings::PluginSettingData;
use crate::{error::ThothError, plugin::Plugin};

pub struct PluginManager {
    engine: Engine,
    /// Interior-mutable so settings can be updated after init without reinitialising the manager.
    plugin_settings: RwLock<HashMap<String, Vec<PluginSettingData>>>,
    pub registry: PluginRegistry,
}

impl std::fmt::Debug for PluginManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginManager")
            .field("registry", &self.registry)
            .finish_non_exhaustive()
    }
}

impl PluginManager {
    pub fn init(plugin_settings: &HashMap<String, Vec<PluginSettingData>>) -> Result<Self> {
        let notification_id = NotificationManager::notify(
            Notification::new("Loading Plugins", "Initializing plugin system...")
                .with_status_bar(true)
                .with_toast(false)
                .with_status(NotificationStatus::Running),
        );

        let mut config = Config::new();
        config.consume_fuel(true);
        match Cache::new(CacheConfig::new()) {
            Ok(cache) => {
                config.cache(Some(cache));
            }
            Err(e) => {
                eprintln!("Plugin cache init failed, proceeding without compilation cache: {e}");
            }
        }
        let engine = Engine::new(&config).map_err(|e| ThothError::PluginLoadError {
            path: PathBuf::new(),
            reason: e.to_string(),
        })?;
        let mut manager = Self {
            engine,
            plugin_settings: RwLock::new(plugin_settings.clone()),
            registry: PluginRegistry::new(),
        };
        manager.scan_all_directories()?;

        NotificationManager::mark_notification_as_complete(&notification_id);

        Ok(manager)
    }

    /// Update the persisted plugin settings. Affects all subsequent plugin opens;
    /// already-running plugin instances should be notified via `on_setting_change`.
    pub fn update_plugin_settings(&self, settings: HashMap<String, Vec<PluginSettingData>>) {
        if let Ok(mut guard) = self.plugin_settings.write() {
            *guard = settings;
        }
    }

    /// Expose the shared wasmtime engine so callers can instantiate plugins
    /// for settings-only use (via `WasmPluginSettings::new`).
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn get_plugin_by_id(&self, id: &str) -> Option<&Plugin> {
        self.registry.get_by_id(id)
    }

    pub fn get_all_plugin(&self) -> Vec<&Plugin> {
        self.registry.get_all_plugins()
    }

    pub fn get_all_plugin_by_capability(&self, capability: Capability) -> Vec<&Plugin> {
        self.registry.get_by_capability(capability)
    }

    /// Return the wasm path for the first FileLoader plugin that declares support
    /// for `ext` (lowercase, no leading dot). Returns `None` if no plugin handles it.
    pub fn find_loader_for_extension(&self, ext: &str) -> Option<PathBuf> {
        let plugin = self.registry.find_loader_for_extension(ext)?;
        plugin.location.as_deref().map(PathBuf::from)
    }

    /// Uninstall a user-installed plugin by id: removes its directory from disk
    /// and deregisters it from the registry. Bundled plugins are rejected.
    pub fn uninstall_plugin(&mut self, id: &str) -> Result<()> {
        let plugin = self
            .registry
            .get_by_id(id)
            .ok_or_else(|| ThothError::Unknown {
                message: format!("Plugin '{id}' not found"),
            })?;

        if plugin.bundled {
            return Err(ThothError::Unknown {
                message: format!("Plugin '{id}' is bundled and cannot be uninstalled"),
            });
        }

        // location holds the full path to plugin.wasm — delete the parent dir
        if let Some(location) = &plugin.location {
            let wasm_path = std::path::Path::new(location);
            if let Some(plugin_dir) = wasm_path.parent()
                && plugin_dir.exists()
            {
                std::fs::remove_dir_all(plugin_dir).map_err(|e| ThothError::Unknown {
                    message: format!("Failed to delete plugin directory: {e}"),
                })?;
            }
        }

        self.registry.remove_plugin(id);
        Ok(())
    }

    /// Returns true if the plugin registered for `ext` declares `capability`.
    pub fn plugin_has_capability(&self, ext: &str, capability: &Capability) -> bool {
        self.registry
            .find_loader_for_extension(ext)
            .map(|p| p.capabilities.contains(capability))
            .unwrap_or(false)
    }

    /// Open `file_path` using a plugin that implements both file-loader and file-viewer.
    /// Returns an error if no plugin is registered for that extension.
    pub fn open_file_with_viewer(
        &self,
        ext: &str,
        file_path: &Path,
    ) -> Result<WasmFileViewerLoader> {
        let wasm_path = self
            .find_loader_for_extension(ext)
            .ok_or_else(|| ThothError::Unknown {
                message: format!("No plugin registered for .{ext}"),
            })?;
        WasmFileViewerLoader::open(&self.engine, &wasm_path, file_path)
    }

    pub fn open_file(&self, ext: &str, file_path: &Path) -> Result<WasmFileLoader> {
        let plugin = self
            .registry
            .find_loader_for_extension(ext)
            .ok_or_else(|| ThothError::Unknown {
                message: format!("No plugin registered for .{ext}"),
            })?;
        let wasm_path = plugin
            .location
            .as_deref()
            .map(std::path::Path::new)
            .ok_or_else(|| ThothError::Unknown {
                message: "Plugin has no wasm path".into(),
            })?;
        let settings_json = {
            let guard = self
                .plugin_settings
                .read()
                .unwrap_or_else(|e| e.into_inner());
            let settings = guard.get(&plugin.id).cloned().unwrap_or_default();
            serde_json::to_string(&settings).unwrap_or_default()
        };

        let mut loader = WasmFileLoader::open(&self.engine, wasm_path, file_path)?;
        loader
            .on_load(&settings_json)
            .map_err(|e| ThothError::PluginLoadError {
                path: wasm_path.to_path_buf(),
                reason: e.to_string(),
            })?;
        Ok(loader)
    }

    pub fn get_data_source_plugins(&self) -> Vec<&Plugin> {
        self.registry.get_by_capability(Capability::DataSource)
    }

    pub fn open_data_source(
        &self,
        plugin_id: &str,
        policy: NetworkPolicy,
    ) -> Result<WasmDataSourceLoader> {
        let plugin = self
            .registry
            .get_by_id(plugin_id)
            .ok_or_else(|| ThothError::Unknown {
                message: format!("Plugin '{plugin_id}' not found"),
            })?;
        let wasm_path = plugin
            .location
            .as_deref()
            .map(std::path::Path::new)
            .ok_or_else(|| ThothError::Unknown {
                message: "Plugin has no wasm path".into(),
            })?;
        let settings = {
            let guard = self
                .plugin_settings
                .read()
                .unwrap_or_else(|e| e.into_inner());
            guard.get(plugin_id).cloned().unwrap_or_default()
        };
        WasmDataSourceLoader::open(
            &self.engine,
            wasm_path,
            policy,
            plugin_id.to_string(),
            &settings,
        )
    }

    /// Instantiate a pure `ui-component` plugin (the `ui-component-plugin` world).
    /// Unlike `open_data_source` there is no network policy — these plugins have
    /// no http-client import.
    pub fn open_ui_component(
        &self,
        plugin_id: &str,
    ) -> Result<crate::plugin::wasm_ui_component::WasmUiComponentLoader> {
        let plugin = self
            .registry
            .get_by_id(plugin_id)
            .ok_or_else(|| ThothError::Unknown {
                message: format!("Plugin '{plugin_id}' not found"),
            })?;
        if !plugin.capabilities.contains(&Capability::NewUIComponent) {
            return Err(ThothError::Unknown {
                message: format!("Plugin '{plugin_id}' does not provide a ui-component"),
            });
        }
        let wasm_path = plugin
            .location
            .as_deref()
            .map(std::path::Path::new)
            .ok_or_else(|| ThothError::Unknown {
                message: "Plugin has no wasm path".into(),
            })?;
        let settings = {
            let guard = self
                .plugin_settings
                .read()
                .unwrap_or_else(|e| e.into_inner());
            guard.get(plugin_id).cloned().unwrap_or_default()
        };
        crate::plugin::wasm_ui_component::WasmUiComponentLoader::open(
            &self.engine,
            wasm_path,
            plugin_id.to_string(),
            &settings,
        )
    }

    /// All plugins that declare the `new-ui-component` capability but are NOT
    /// data sources (those are listed separately in the sidebar).
    pub fn get_ui_component_plugins(&self) -> Vec<&Plugin> {
        self.registry
            .get_by_capability(Capability::NewUIComponent)
            .into_iter()
            .filter(|p| !p.capabilities.contains(&Capability::DataSource))
            .collect()
    }

    pub fn open_plugin_settings(&self, plugin_id: &str) -> Result<WasmPluginSettings> {
        let plugin = self
            .registry
            .get_by_id(plugin_id)
            .ok_or_else(|| ThothError::Unknown {
                message: format!("Plugin '{plugin_id}' not found"),
            })?;

        if plugin.capabilities.contains(&Capability::Theme) {
            return Err(ThothError::PluginLoadError {
                path: PathBuf::from(plugin.location.clone().unwrap_or("".to_string())),
                reason: "Trying to load wasm for theme plugin".to_string(),
            });
        }

        let wasm_path = plugin
            .location
            .as_deref()
            .map(std::path::Path::new)
            .ok_or_else(|| ThothError::Unknown {
                message: "Plugin has no wasm path".into(),
            })?;

        WasmPluginSettings::new(&self.engine, wasm_path)
    }

    pub fn get_installed_plugin() -> HashMap<String, Plugin> {
        if let Some(Some(pm)) = PLUGIN_MANAGER.get() {
            return pm.registry.get_installed_plugins();
        }

        HashMap::new()
    }

    fn scan_all_directories(&mut self) -> Result<()> {
        for (dir, is_bundled) in self.plugin_directories() {
            if let Ok(dir) = dir
                && dir.exists()
            {
                eprintln!("Checking {}", dir.display());
                if let Err(e) = self.scan_directory(dir, is_bundled) {
                    eprintln!("Failed to scan plugin directory: {e:?}");
                }
            }
        }

        // Marketplace-installed plugins live in marketplace/installs/{id}/.
        // scan_directory can't be reused here because it appends "plugins" —
        // call scan_instances_dir directly instead.
        if let Ok(installs_dir) = PersistentState::plugin_install_dir()
            && installs_dir.exists()
        {
            eprintln!("Checking marketplace installs {}", installs_dir.display());
            if let Err(e) = self.scan_instances_dir(&installs_dir, false) {
                eprintln!("Failed to scan marketplace installs: {e:?}");
            }
        }

        Ok(())
    }

    /// Returns `(directory, is_bundled)` pairs for directories whose layout is
    /// `{dir}/plugins/{id}/plugin.toml`. Marketplace plugins are handled
    /// separately in `scan_all_directories` via `scan_instances_dir`.
    fn plugin_directories(&self) -> Vec<(Result<PathBuf>, bool)> {
        let mut dirs = vec![(self.bundled_plugins_dir(), true)];

        // In debug builds, also check the workspace source tree so `cargo run`
        // finds plugins without a full install. option_env! is resolved at
        // compile time, so CARGO_MANIFEST_DIR is always available here.
        #[cfg(debug_assertions)]
        if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
            dirs.push((Ok(PathBuf::from(manifest_dir).join("assets")), true));
        }

        dirs
    }

    fn bundled_plugins_dir(&self) -> Result<PathBuf> {
        let exe = env::current_exe().map_err(|_| ThothError::PluginDirectoryInvalid {
            dir: "Bundled".to_string(),
        })?;
        let exe_dir = exe
            .parent()
            .ok_or_else(|| ThothError::PluginDirectoryInvalid {
                dir: "Bundled".to_string(),
            })?;

        // cargo-packager copies resources/ next to the exe on Linux/Windows,
        // and into Contents/Resources/ on macOS.
        #[cfg(target_os = "macos")]
        let base = exe_dir.join("../Resources");
        #[cfg(not(target_os = "macos"))]
        let base = exe_dir.to_path_buf();

        // Return the base assets dir — scan_directory will append "plugins".
        Ok(base.join("assets"))
    }

    pub fn scan_directory(&mut self, dir: PathBuf, is_bundled: bool) -> Result<()> {
        let plugin_dir = dir.join("plugins");
        if !plugin_dir.exists() {
            eprintln!("No plugin Directory at {}", plugin_dir.display());
            return Ok(());
        }
        self.scan_instances_dir(&plugin_dir, is_bundled)
    }

    /// Scan a directory whose immediate children are plugin instance directories
    /// (each containing `plugin.toml` + `plugin.wasm` or `theme.json`).
    /// Unlike `scan_directory`, this takes the instances dir directly without
    /// appending "plugins".
    fn scan_instances_dir(&mut self, plugin_dir: &std::path::Path, is_bundled: bool) -> Result<()> {
        for entry in std::fs::read_dir(plugin_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let toml_path = path.join("plugin.toml");
            let plugin_path = path.join("plugin.wasm");
            let theme_path = path.join("theme.json");

            let contents = match std::fs::read_to_string(&toml_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!(
                        "Skipping plugin at {}: failed to read plugin.toml: {e}",
                        path.display()
                    );
                    continue;
                }
            };

            let mut plugin: Plugin = match toml::from_str(&contents) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!(
                        "Skipping plugin at {}: invalid plugin.toml: {e}",
                        toml_path.display()
                    );
                    continue;
                }
            };

            // Set bundled flag before load_plugin() so it survives the
            // location overwrite inside load_plugin(). Only plugins from the
            // bundled directory are immutable; user-installed ones are not.
            plugin.bundled = is_bundled;

            let icon = path.join("icon.png");
            if icon.exists() {
                plugin.icon_path = Some(icon);
            }

            if theme_path.exists() {
                plugin.location = Some(theme_path.display().to_string());
                self.registry.add_plugin(plugin);
            } else if let Err(e) = self.load_plugin(plugin_path.clone(), plugin) {
                eprintln!("Skipping plugin at {}: {e}", plugin_path.display());
            }
        }

        Ok(())
    }

    fn load_plugin(&mut self, wasm_path: PathBuf, mut meta: Plugin) -> Result<()> {
        struct PluginState {
            wasi: WasiCtx,
            table: ResourceTable,
        }

        impl wasmtime_wasi::WasiView for PluginState {
            fn ctx(&mut self) -> WasiCtxView<'_> {
                WasiCtxView {
                    ctx: &mut self.wasi,
                    table: &mut self.table,
                }
            }
        }

        let wasi = WasiCtxBuilder::new().build();
        let mut store = Store::new(
            &self.engine,
            PluginState {
                wasi,
                table: ResourceTable::new(),
            },
        );
        store
            .set_fuel(1_000_000)
            .map_err(|e| ThothError::PluginLoadError {
                path: wasm_path.clone(),
                reason: e.to_string(),
            })?;

        let component = wasmtime::component::Component::from_file(&self.engine, &wasm_path)
            .map_err(|e| ThothError::PluginLoadError {
                path: wasm_path.clone(),
                reason: e.to_string(),
            })?;

        // Validate: check the component exports the required plugin-meta interface
        // before spending time linking and instantiating.
        // Export names include a semver suffix, e.g. "thoth:plugin/plugin-meta@0.1.0"
        let has_plugin_meta = component
            .component_type()
            .exports(&self.engine)
            .any(|(name, _)| name.starts_with("thoth:plugin/plugin-meta"));

        if !has_plugin_meta {
            return Err(ThothError::PluginLoadError {
                path: wasm_path,
                reason: "missing required export: thoth:plugin/plugin-meta".to_string(),
            });
        }

        // Link WASI host functions so the plugin can use them
        let mut linker = wasmtime::component::Linker::<PluginState>::new(&self.engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| {
            ThothError::PluginLoadError {
                path: wasm_path.clone(),
                reason: e.to_string(),
            }
        })?;

        // Stub out any imports not already provided (e.g. http-client for
        // data-source plugins). The stubs trap if called — they exist only to
        // satisfy the linker during this metadata-read instantiation.
        linker
            .define_unknown_imports_as_traps(&component)
            .map_err(|e| ThothError::PluginLoadError {
                path: wasm_path.clone(),
                reason: e.to_string(),
            })?;

        // Instantiate — this validates that all imports are satisfied
        linker
            .instantiate(&mut store, &component)
            .map_err(|e| ThothError::PluginLoadError {
                path: wasm_path.clone(),
                reason: e.to_string(),
            })?;

        // Full type-safe invocation of plugin-meta.get-info and plugin-lifecycle.on-load
        // requires wit-bindgen generated bindings (see docs/PLUGIN_SYSTEM.md, Step 2).
        meta.location = Some(wasm_path.display().to_string());
        self.registry.add_plugin(meta);
        Ok(())
    }
}
