//! Thoth core
//!
//! Display-independent logic shared by the desktop UI (`thoth-ui`) and the
//! command-line interface (`thoth-cli`): file loading, the Papyrus data bus,
//! the sandboxed WebAssembly plugin runtime, MCP, settings and updates.
//!
//! This crate must not depend on `eframe`/`egui`; anything that needs a
//! graphics context lives in `thoth-ui` and reaches back through the seams in
//! [`repaint`].

use std::sync::{OnceLock, atomic::AtomicBool};

pub mod config_dir;
pub mod consent;
pub mod constants;
pub mod core;
pub mod dialogs;
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

/// Set to `true` by the "Update Now" notification action callback.
/// Polled each frame by ThothApp and cleared after opening the updates settings tab.
pub static OPEN_UPDATES_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Request a repaint from display-independent code.
///
/// `thoth-ui` installs the real notifier (an `egui::Context::request_repaint`)
/// once on startup; headless callers leave it unset, so this is a no-op.
pub mod repaint {
    use std::sync::Arc;
    use std::sync::OnceLock;

    type Notifier = Arc<dyn Fn() + Send + Sync + 'static>;

    static NOTIFIER: OnceLock<Notifier> = OnceLock::new();

    /// Install the process-wide repaint notifier. Ignored if already set.
    pub fn set_notifier(notifier: Notifier) {
        let _ = NOTIFIER.set(notifier);
    }

    /// Ask the host to repaint, if one is attached.
    pub fn request() {
        if let Some(notifier) = NOTIFIER.get() {
            notifier();
        }
    }
}
