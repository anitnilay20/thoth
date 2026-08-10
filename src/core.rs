//! Display-independent application state and event processing.
//!
//! [`ThothCore`] can be constructed and ticked without creating an egui context
//! or a native window. The desktop app owns one core and applies the actions it
//! emits to its window-specific state. A headless runtime can drive the same
//! event queue directly.

use std::{collections::VecDeque, path::PathBuf};

use crate::{
    app::{persistent_state::PersistentState, tab_manager::TabId},
    plugin::{datasets::DatasetStore, runtime::PluginRuntime},
    settings::Settings,
    state::ApplicationUpdateState,
};

/// An input delivered to the application core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreEvent {
    /// Request that the host open a local file.
    OpenFile(PathBuf),
}

/// Work emitted by [`ThothCore::tick`] for a host runtime to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreAction {
    /// Open a local file in the active host surface.
    OpenFile(PathBuf),
}

/// Non-render state shared by GUI and future headless runtimes.
pub struct ThothCore {
    pub settings: Settings,
    pub persistent_state: PersistentState,
    pub update_state: ApplicationUpdateState,
    /// Asynchronously initialized plugin manager owned by the application core.
    pub plugins: PluginRuntime,
    /// Host-owned dataset registry shared with plugin WIT and SDK callbacks.
    pub datasets: DatasetStore,
    pub(crate) settings_changed: bool,
    pub(crate) session_dirty: bool,
    pub(crate) pending_plugin_restores: Vec<(String, Option<String>)>,
    pub(crate) session_restore_active_index: Option<usize>,
    pub(crate) last_active_plugin_tab: Option<TabId>,
    pub(crate) chart_counter: usize,
    pub(crate) chart_source: Option<(TabId, Vec<String>, Vec<Vec<String>>)>,
    events: VecDeque<CoreEvent>,
}

impl ThothCore {
    /// Initialize application state without creating a graphics context or window.
    pub fn init(settings: Settings) -> Self {
        Self::with_persistent_state(settings, PersistentState::default())
    }

    pub(crate) fn with_persistent_state(
        settings: Settings,
        persistent_state: PersistentState,
    ) -> Self {
        Self {
            settings,
            persistent_state,
            update_state: ApplicationUpdateState::default(),
            plugins: PluginRuntime::new(),
            datasets: DatasetStore::new(),
            settings_changed: false,
            session_dirty: false,
            pending_plugin_restores: Vec::new(),
            session_restore_active_index: None,
            last_active_plugin_tab: None,
            chart_counter: 0,
            chart_source: None,
            events: VecDeque::new(),
        }
    }

    /// Queue an event for processing on the next tick.
    pub fn dispatch_event(&mut self, event: CoreEvent) {
        self.events.push_back(event);
    }

    /// Process all queued events without blocking and return host actions.
    pub fn tick(&mut self) -> Vec<CoreAction> {
        self.events
            .drain(..)
            .map(|event| match event {
                CoreEvent::OpenFile(path) => CoreAction::OpenFile(path),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{CoreAction, CoreEvent, ThothCore};

    #[test]
    fn initializes_and_ticks_without_a_display() {
        let mut core = ThothCore::init(crate::settings::Settings::default());
        let path = std::path::PathBuf::from("headless.json");

        core.dispatch_event(CoreEvent::OpenFile(path.clone()));

        assert_eq!(core.tick(), vec![CoreAction::OpenFile(path)]);
        assert!(core.tick().is_empty());
    }
}
