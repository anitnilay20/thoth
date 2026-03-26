use std::env;
use std::path::PathBuf;

use wasmtime::component::ResourceTable;
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView};

use crate::error::Result;
use crate::plugin::Capability;
use crate::plugin::plugin_registry::PluginRegistry;
use crate::{error::ThothError, plugin::Plugin};

#[derive(Debug, Default)]
pub struct PluginManager {
    engine: Engine,
    registry: PluginRegistry,
}

// #[allow(dead_code)]
impl PluginManager {
    pub fn init() -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config).map_err(|e| ThothError::PluginLoadError {
            path: PathBuf::new(),
            reason: e.to_string(),
        })?;
        let mut manager = Self {
            engine,
            registry: PluginRegistry::new(),
        };
        manager.scan_all_directories()?;
        Ok(manager)
    }

    pub fn get_all_plugin_by_capability(&self, capability: Capability) -> Vec<&Plugin> {
        self.registry.get_by_capability(capability)
    }

    fn scan_all_directories(&mut self) -> Result<()> {
        for dir in self.plugin_directories() {
            if let Ok(dir) = dir
                && dir.exists()
            {
                println!("Checking {}", dir.display());
                self.scan_directory(dir)?;
            }
        }

        Ok(())
    }

    fn plugin_directories(&self) -> Vec<Result<PathBuf>> {
        let mut dirs = vec![self.bundled_plugins_dir(), self.user_plugin_dir()];

        // In debug builds, also check the workspace source tree so `cargo run`
        // finds plugins without a full install. option_env! is resolved at
        // compile time, so CARGO_MANIFEST_DIR is always available here.
        #[cfg(debug_assertions)]
        if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
            dirs.push(Ok(PathBuf::from(manifest_dir).join("assets")));
        }

        dirs
    }

    fn bundled_plugins_dir(&self) -> Result<PathBuf> {
        let exe = env::current_exe().map_err(|_| ThothError::PluginDirectoryInvalid {
            dir: "Bundled".to_string(),
        })?;
        let exe_dir = exe.parent().ok_or_else(|| ThothError::PluginDirectoryInvalid {
            dir: "Bundled".to_string(),
        })?;

        // cargo-packager copies resources/ next to the exe on Linux/Windows,
        // and into Contents/Resources/ on macOS.
        #[cfg(target_os = "macos")]
        let base = exe_dir.join("../Resources");
        #[cfg(not(target_os = "macos"))]
        let base = exe_dir.to_path_buf();

        Ok(base.join("assets/plugins"))
    }

    fn user_plugin_dir(&self) -> Result<PathBuf> {
        let config_dir = dirs::config_dir().ok_or_else(|| ThothError::SettingsLoadError {
            reason: "Failed to get config directory".to_string(),
        })?;

        Ok(config_dir.join("thoth"))
    }

    // fn load_plugin_exe(path: PathBuf)

    pub fn scan_directory(&mut self, dir: PathBuf) -> Result<()> {
        let plugin_dir = dir.join("plugins");

        if !plugin_dir.exists() {
            eprintln!("No plugin Directory at {}", plugin_dir.display());
            return Ok(());
        }

        for entry in std::fs::read_dir(plugin_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let toml_path = path.join("plugin.toml");
                let plugin_path = path.join("plugin.wasm");
                let contents = std::fs::read_to_string(toml_path.clone())?;
                let mut plugin: Plugin =
                    toml::from_str(&contents).map_err(|e| ThothError::PluginLoadError {
                        path: toml_path,
                        reason: e.to_string(),
                    })?;
                plugin.location = Some("BUNDLE".to_string());
                let icon = path.join("icon.png");
                if icon.exists() {
                    plugin.icon_path = Some(icon);
                }
                // self.registry.add_plugin(plugin);
                self.load_plugin(plugin_path, plugin)?;
            } else {
                // Ignore
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
