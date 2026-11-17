//! Thoth desktop UI
//!
//! The `eframe`/`egui` application. Display-independent logic is re-exported
//! from `thoth-core` (and the command runtime from `thoth-cli`) so existing
//! `thoth::file`, `thoth::settings`, `thoth::cli` paths keep working.

pub use thoth_cli::{cli, headless};
pub use thoth_core::*;

pub mod app;
pub mod components;
pub mod consent;
pub mod helpers;
pub mod notification;
pub mod platform;
pub mod plugin;
pub mod settings_ext;
pub mod shortcuts;
pub mod state;
pub mod theme;

pub use settings_ext::SettingsCtx;

#[cfg(test)]
mod test_support;

/// The egui context, published once on the first frame so background workers
/// (e.g. the WebSocket connection tasks) can request a repaint when data
/// arrives off the UI thread.
pub static EGUI_CTX: OnceLock<eframe::egui::Context> = OnceLock::new();

use std::sync::OnceLock;

/// Install the core seams that need an egui context: repaint notifications and
/// the native file picker used by the plugin `file-dialog` import.
pub fn install_host_seams(ctx: &eframe::egui::Context) {
    let _ = EGUI_CTX.set(ctx.clone());

    let repaint_ctx = ctx.clone();
    thoth_core::repaint::set_notifier(std::sync::Arc::new(move || repaint_ctx.request_repaint()));

    thoth_core::dialogs::set_picker(std::sync::Arc::new(
        |request: thoth_core::dialogs::FileDialogRequest| {
            let mut dialog = rfd::FileDialog::new();
            if !request.title.is_empty() {
                dialog = dialog.set_title(&request.title);
            }
            if !request.extensions.is_empty() {
                let exts: Vec<&str> = request.extensions.iter().map(String::as_str).collect();
                dialog = dialog.add_filter("", &exts);
            }
            if let Some(name) = &request.default_name {
                dialog = dialog.set_file_name(name);
            }
            if request.save {
                dialog.save_file()
            } else {
                dialog.pick_file()
            }
        },
    ));
}
