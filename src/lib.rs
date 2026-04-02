//! Thoth library
//!
//! This library exposes the core components and utilities of Thoth
//! for testing and potential reuse.

use std::sync::OnceLock;

use crate::plugin::manager::PluginManager;

pub mod app;
pub mod components;
pub mod constants;
pub mod error;
pub mod file;
pub mod helpers;
pub mod platform;
pub mod plugin;
pub mod search;
pub mod settings;
pub mod shortcuts;
pub mod state;
pub mod theme;
pub mod update;
pub mod notification;

pub static PLUGIN_MANAGER: OnceLock<Option<PluginManager>> = OnceLock::new();
pub static NOTIFICATION_MANAGER: OnceLock<std::sync::Mutex<notification::NotificationManager>> = OnceLock::new();
