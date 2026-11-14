//! Thoth core
//!
//! Display-independent logic shared by the desktop UI and the command-line
//! interface: file loading, the Papyrus data bus, the sandboxed WebAssembly
//! plugin runtime, MCP, settings and updates.
//!
//! The CLI lives in `thoth-cli`; the `thoth` host crate re-exports this crate
//! and adds the desktop UI.

use std::sync::{OnceLock, atomic::AtomicBool};

pub mod config_dir;
pub mod consent;
pub mod constants;
pub mod core;
pub mod error;
pub mod file;
pub mod helpers;
pub mod mcp;
pub mod notification;
pub mod papyrus;
pub mod persistent_state;
pub mod platform;
pub mod plugin;
pub mod settings;
pub mod shortcuts;
pub mod state;
pub mod theme;
pub mod update;

pub static NOTIFICATION_MANAGER: OnceLock<std::sync::Mutex<notification::NotificationManager>> =
    OnceLock::new();
pub static CONSENT_MANAGER: OnceLock<std::sync::Mutex<consent::manager::ConsentManager>> =
    OnceLock::new();

/// The egui context, published once on the first frame so background workers
/// (e.g. the WebSocket connection tasks) can request a repaint when data
/// arrives off the UI thread.
pub static EGUI_CTX: OnceLock<eframe::egui::Context> = OnceLock::new();

/// Set to `true` by the "Update Now" notification action callback.
/// Polled each frame by ThothApp and cleared after opening the updates settings tab.
pub static OPEN_UPDATES_REQUESTED: AtomicBool = AtomicBool::new(false);
