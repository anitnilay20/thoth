#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Set up dhat allocator for memory profiling (only when profiling feature is enabled)
#[cfg(feature = "profiling")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use eframe::{App, Frame, NativeOptions, egui};
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

/// Standalone settings window app
struct SettingsWindow {
    settings_dialog: components::settings_dialog::SettingsDialog,
    settings: settings::Settings,
}

impl SettingsWindow {
    fn new(settings: settings::Settings) -> Self {
        let mut settings_dialog = components::settings_dialog::SettingsDialog::default();
        settings_dialog.open(&settings);
        Self {
            settings_dialog,
            settings,
        }
    }
}

impl App for SettingsWindow {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // Apply theme
        theme::apply_theme(ctx, &self.settings);

        // Show settings dialog
        if let Some(new_settings) = self.settings_dialog.show(ctx) {
            // Save settings to disk
            if let Err(e) = new_settings.save() {
                eprintln!("Failed to save settings: {}", e);
            } else {
                eprintln!("Settings saved successfully");
            }
            self.settings = new_settings;

            // Close the window after saving
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

/// Run the settings window
fn run_settings_window(settings: settings::Settings) -> Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_title("Thoth Settings")
            .with_resizable(true),
        ..Default::default()
    };

    let result = eframe::run_native(
        "Thoth Settings",
        options,
        Box::new(move |cc| {
            // Initialize Phosphor icon fonts
            let mut fonts = egui::FontDefinitions::default();
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(SettingsWindow::new(settings)))
        }),
    );

    if let Err(e) = result {
        eprintln!("Error running settings window: {e:?}");
        return Err("Failed to run settings window".into());
    }
    Ok(())
}

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

    // Check if launched in settings mode
    let args: Vec<String> = std::env::args().collect();
    let is_settings_mode = args.iter().any(|arg| arg == "--settings");

    // Load settings first
    let settings = settings::Settings::load().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load settings: {}. Using defaults.", e);
        settings::Settings::default()
    });

    // If in settings mode, launch settings window instead of main app
    if is_settings_mode {
        return run_settings_window(settings);
    }

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
        return Err("Failed to run application".into());
    }
    Ok(())
}
