//! egui presentation of notifications. The store lives in `thoth-core`; this
//! module re-exports it and renders queued toasts plus the dropdown.

pub use thoth_core::notification::{
    Notification, NotificationKind, NotificationManager, NotificationStatus,
};

pub mod notification_dropdown;

use eframe::egui;
use egui_notify::Toasts;
use std::sync::{Mutex, OnceLock};

fn toasts() -> &'static Mutex<Toasts> {
    static TOASTS: OnceLock<Mutex<Toasts>> = OnceLock::new();
    TOASTS
        .get_or_init(|| Mutex::new(Toasts::default().with_anchor(egui_notify::Anchor::BottomRight)))
}

/// Drain queued toasts from the core manager and render them.
pub fn show_notifications(ctx: &egui::Context) {
    let (pending, dismiss) = match crate::NOTIFICATION_MANAGER
        .get()
        .and_then(|manager| manager.lock().ok())
    {
        Some(mut nm) => (
            nm.pending_toasts.drain(..).collect::<Vec<_>>(),
            std::mem::take(&mut nm.dismiss_toasts),
        ),
        None => (Vec::new(), false),
    };

    let mut t = toasts().lock().unwrap_or_else(|e| e.into_inner());
    // Dismiss before adding, so a clear request cannot remove toasts raised in
    // the same frame.
    if dismiss {
        t.dismiss_all_toasts();
    }
    for title in pending {
        t.info(title)
            .closable(true)
            .duration(std::time::Duration::from_secs(5));
    }
    t.show(ctx);
}
