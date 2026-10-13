//! egui-memory helpers for [`Settings`].
//!
//! The serialisable settings data lives in `thoth-core`; stashing it in the
//! egui context so any widget can read and mutate it is a display concern.

use eframe::egui;
use thoth_core::settings::Settings;

const CTX_ID: &str = "app_settings";
const CTX_DIRTY_ID: &str = "app_settings_dirty";

/// Store/read/mutate [`Settings`] through the egui context's temporary memory.
pub trait SettingsCtx {
    /// Write settings into the egui context. Call once at the top of each frame.
    fn store(ctx: &egui::Context, settings: &Self);

    /// Read settings from the egui context, falling back to defaults.
    fn read(ctx: &egui::Context) -> Self;

    /// Mutate settings in the egui context. Any component can call this;
    /// `ThothApp` picks up the change at end of frame.
    fn update(ctx: &egui::Context, f: impl FnOnce(&mut Self));

    /// Returns `Some(settings)` if any component called `update()` this frame,
    /// resetting the dirty flag.
    fn take_if_dirty(ctx: &egui::Context) -> Option<Self>
    where
        Self: Sized;
}

impl SettingsCtx for Settings {
    fn store(ctx: &egui::Context, settings: &Self) {
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new(CTX_ID), settings.clone());
            d.insert_temp(egui::Id::new(CTX_DIRTY_ID), false);
        });
    }

    fn read(ctx: &egui::Context) -> Self {
        ctx.data(|d| d.get_temp(egui::Id::new(CTX_ID)).unwrap_or_default())
    }

    fn update(ctx: &egui::Context, f: impl FnOnce(&mut Self)) {
        ctx.data_mut(|d| {
            let mut s: Self = d.get_temp(egui::Id::new(CTX_ID)).unwrap_or_default();
            f(&mut s);
            d.insert_temp(egui::Id::new(CTX_ID), s);
            d.insert_temp(egui::Id::new(CTX_DIRTY_ID), true);
        });
    }

    fn take_if_dirty(ctx: &egui::Context) -> Option<Self> {
        let dirty: bool = ctx
            .data(|d| d.get_temp(egui::Id::new(CTX_DIRTY_ID)))
            .unwrap_or(false);
        if dirty {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new(CTX_DIRTY_ID), false));
            ctx.data(|d| d.get_temp(egui::Id::new(CTX_ID)))
        } else {
            None
        }
    }
}
