#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Set up dhat allocator for memory profiling (only when profiling feature is enabled)
#[cfg(feature = "profiling")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

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
    // Initialize dhat heap profiler (only when profiling feature is enabled)
    // When the app exits, dhat writes 'dhat-heap.json' which can be viewed at:
    // https://nnethercote.github.io/dh_view/dh_view.html
    // This shows per-component memory allocations with full call stacks
    #[cfg(feature = "profiling")]
    let _profiler = dhat::Profiler::new_heap();

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
            ])
            .with_decorations(false), // Remove native window decorations for custom title bar
        ..Default::default()
    };

    let result = eframe::run_native(
        "Thoth — JSON & NDJSON Viewer",
        options,
        Box::new(move |cc| {
            // Initialize Phosphor icon fonts
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(app::ThothApp::new(settings)))
        }),
    );

    // When profiling is enabled, remind user about dhat output
    #[cfg(feature = "profiling")]
    eprintln!("\n📊 Profiling data saved to dhat-heap.json");
    #[cfg(feature = "profiling")]
    eprintln!("   View at: https://nnethercote.github.io/dh_view/dh_view.html\n");

    if let Err(e) = result {
        eprintln!("Error running application: {e:?}");
        return Err(anyhow::anyhow!("Failed to run application"));
    }
    Ok(())
}
