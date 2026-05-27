//! Thoth library
//!
//! This library exposes the core components and utilities of Thoth
//! for testing and potential reuse.

use std::sync::{OnceLock, atomic::AtomicBool};

use crate::plugin::manager::PluginManager;

pub mod app;
pub mod components;
pub mod consent;
pub mod constants;
pub mod error;
pub mod file;
pub mod helpers;
pub mod mcp;
pub mod notification;
pub mod platform;
pub mod plugin;
pub mod search;
pub mod settings;
pub mod shortcuts;
pub mod state;
pub mod theme;
pub mod update;

pub static PLUGIN_MANAGER: OnceLock<Option<PluginManager>> = OnceLock::new();
pub static NOTIFICATION_MANAGER: OnceLock<std::sync::Mutex<notification::NotificationManager>> =
    OnceLock::new();
pub static CONSENT_MANAGER: OnceLock<std::sync::Mutex<consent::manager::ConsentManager>> =
    OnceLock::new();

/// Set to `true` by the "Update Now" notification action callback.
/// Polled each frame by ThothApp and cleared after opening the updates settings tab.
pub static OPEN_UPDATES_REQUESTED: AtomicBool = AtomicBool::new(false);
