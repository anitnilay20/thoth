//! Shared ownership and asynchronous initialization for the plugin manager.
//!
//! The application core owns [`PluginRuntime`]. A weak process bridge is kept
//! only for legacy host callbacks that cannot receive state directly (for
//! example SDK-installed function pointers); it does not keep the runtime alive.

use std::sync::{
    Arc, LazyLock, RwLock, Weak,
    atomic::{AtomicBool, Ordering},
};

use crate::{plugin::manager::PluginManager, settings::PluginSettingData};

type SharedState = Arc<RwLock<PluginRuntimeState>>;

/// Current plugin-manager initialization state.
#[derive(Clone)]
pub enum PluginRuntimeState {
    /// Initialization has not completed yet.
    Loading,
    /// Plugins are disabled or initialization failed.
    Disabled,
    /// The initialized manager, shared by GUI and headless consumers.
    Ready(Arc<PluginManager>),
}

/// Core-owned plugin-manager runtime.
pub struct PluginRuntime {
    state: SharedState,
    started: Arc<AtomicBool>,
}

impl PluginRuntime {
    /// Create an unstarted plugin runtime.
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(PluginRuntimeState::Loading)),
            started: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start plugin discovery once. Initialization runs off the caller thread.
    pub fn start(
        &self,
        enabled: bool,
        plugin_settings: std::collections::HashMap<String, Vec<PluginSettingData>>,
    ) {
        if self.started.swap(true, Ordering::AcqRel) {
            return;
        }
        if !enabled {
            self.set_state(PluginRuntimeState::Disabled);
            return;
        }

        let state = Arc::clone(&self.state);
        std::thread::spawn(move || {
            let next = match PluginManager::init(&plugin_settings) {
                Ok(manager) => PluginRuntimeState::Ready(Arc::new(manager)),
                Err(err) => {
                    eprintln!("Warning Unable to load plugins: {err}");
                    PluginRuntimeState::Disabled
                }
            };
            if let Ok(mut current) = state.write() {
                *current = next;
            }
        });
    }

    /// Snapshot the current initialization state.
    pub fn state(&self) -> PluginRuntimeState {
        self.state
            .read()
            .map(|state| state.clone())
            .unwrap_or(PluginRuntimeState::Disabled)
    }

    /// Return the initialized manager, if ready.
    pub fn manager(&self) -> Option<Arc<PluginManager>> {
        match self.state() {
            PluginRuntimeState::Ready(manager) => Some(manager),
            PluginRuntimeState::Loading | PluginRuntimeState::Disabled => None,
        }
    }

    /// Install this runtime as the weak bridge used by state-less host callbacks.
    pub fn install_as_active(&self) {
        if let Ok(mut active) = ACTIVE_RUNTIME.write() {
            *active = Arc::downgrade(&self.state);
        }
    }

    fn set_state(&self, state: PluginRuntimeState) {
        if let Ok(mut current) = self.state.write() {
            *current = state;
        }
    }
}

impl Default for PluginRuntime {
    fn default() -> Self {
        Self::new()
    }
}

static ACTIVE_RUNTIME: LazyLock<RwLock<Weak<RwLock<PluginRuntimeState>>>> =
    LazyLock::new(|| RwLock::new(Weak::new()));

/// Resolve the active core's plugin manager for legacy state-less callbacks.
pub fn active_manager() -> Option<Arc<PluginManager>> {
    let state = ACTIVE_RUNTIME.read().ok()?.upgrade()?;
    let snapshot = state.read().ok()?.clone();
    match snapshot {
        PluginRuntimeState::Ready(manager) => Some(manager),
        PluginRuntimeState::Loading | PluginRuntimeState::Disabled => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{PluginRuntime, PluginRuntimeState};

    #[test]
    fn disabled_runtime_finishes_without_starting_a_worker() {
        let runtime = PluginRuntime::new();
        runtime.start(false, Default::default());
        assert!(matches!(runtime.state(), PluginRuntimeState::Disabled));
        assert!(runtime.manager().is_none());
    }
}
