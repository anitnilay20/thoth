#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Set up dhat allocator for memory profiling (only when profiling feature is enabled)
#[cfg(feature = "profiling")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use eframe::{NativeOptions, egui};
use thoth::error::Result;

use crate::helpers::load_icon;

mod app;
mod components;
mod constants;
mod error;
mod file;
mod helpers;
mod platform;
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

    // Note: Settings mode is no longer supported
    // Settings are now opened via viewport mode from the main application
    //
    // Parse simple CLI flags: --help / -h and optional FILE arg
    let mut cli_file: Option<std::path::PathBuf> = None;
    for arg in std::env::args_os().skip(1) {
        if arg == std::ffi::OsString::from("--help") || arg == std::ffi::OsString::from("-h") {
            println!("Thoth — JSON & NDJSON Viewer\n\nUsage: thoth [OPTIONS] [FILE]\n\nOptions:\n  -h, --help    Show this help message\n\nIf a FILE is supplied, Thoth will open it on startup.");
            return Ok(());
        }
        // First non-flag arg is treated as file path
        cli_file = Some(std::path::PathBuf::from(arg));
        break;
    }

    // Load settings first
    let settings = settings::Settings::load().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load settings: {}. Using defaults.", e);
        settings::Settings::default()
    });

    // // If in settings mode, launch settings window instead of main app
    // if is_settings_mode {
    //     return run_settings_window(settings);
    // }

    let icon = load_icon(include_bytes!("../assets/thoth_icon_256.png"));

    // Configure window from settings
    let mut viewport = egui::ViewportBuilder::default();
    if let Some(icon_data) = icon {
        viewport = viewport.with_icon(icon_data);
    }
    let options = NativeOptions {
        viewport: viewport
            .with_inner_size([
                settings.window.default_width,
                settings.window.default_height,
            ])
            // macOS-specific: Unified title bar (like VS Code)
            // This extends content into title bar area, allowing toolbar to share row with traffic lights
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false),
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

            Ok(Box::new(app::ThothApp::new(settings, cli_file)))
        }),
    );

    // When profiling is enabled, remind user about dhat output
    #[cfg(feature = "profiling")]
    eprintln!("\n📊 Profiling data saved to dhat-heap.json");
    #[cfg(feature = "profiling")]
    eprintln!("   View at: https://nnethercote.github.io/dh_view/dh_view.html\n");

    if let Err(e) = result {
        eprintln!("Error running application: {e:?}");
        return Err("Failed to run application".into());
    }
    Ok(())
}
