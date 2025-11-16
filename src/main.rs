#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use eframe::{NativeOptions, egui};

use crate::helpers::load_icon;

mod app;
mod components;
mod file;
mod helpers;
mod search;
mod settings;
mod shortcuts;
mod state;
mod theme;
mod update;

fn main() -> Result<()> {
    // Initialize puffin profiler (only when profiling feature is enabled)
    #[cfg(feature = "profiling")]
    puffin::set_scopes_on(true);

    // Load settings first
    let settings = settings::Settings::load().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load settings: {}. Using defaults.", e);
        settings::Settings::default()
    });

    let icon = load_icon(include_bytes!("../assets/thoth_icon_256.png"));

    // Configure window from settings
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(icon)
            .with_inner_size([
                settings.window.default_width,
                settings.window.default_height,
            ]),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "Thoth — JSON & NDJSON Viewer",
        options,
        Box::new(move |_cc| Ok(Box::new(app::ThothApp::new(settings)))),
    ) {
        eprintln!("Error running application: {e:?}");
        return Err(anyhow::anyhow!("Failed to run application"));
    }
    Ok(())
}
