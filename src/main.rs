#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Set up dhat allocator for memory profiling (only when profiling feature is enabled)
#[cfg(feature = "profiling")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use eframe::{NativeOptions, egui};
use std::path::PathBuf;
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

/// Parse command-line arguments to extract file path
fn parse_file_argument(args: &[String]) -> Result<Option<PathBuf>> {
    // Skip first argument (executable name)
    if args.len() < 2 {
        return Ok(None);
    }

    let file_path_str = &args[1];

    // Validate and sanitize the path
    let path = PathBuf::from(file_path_str);

    // Resolve to absolute path
    let canonical_path = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: Cannot open file '{}': {}", file_path_str, e);
            return Err(format!("Cannot open file '{}': {}", file_path_str, e).into());
        }
    };

    // Verify it's a file (not a directory)
    if !canonical_path.is_file() {
        eprintln!("Error: '{}' is not a file", file_path_str);
        return Err(format!("Not a file: {}", file_path_str).into());
    }

    // Verify file extension is JSON-related
    if let Some(ext) = canonical_path.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        if !matches!(ext_lower.as_str(), "json" | "ndjson" | "jsonl" | "geojson") {
            eprintln!(
                "Warning: File '{}' does not have a JSON extension",
                file_path_str
            );
            // Allow opening anyway - user might know what they're doing
        }
    }

    Ok(Some(canonical_path))
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

    // Parse command-line arguments for file path
    let args: Vec<String> = std::env::args().collect();
    let file_to_open = parse_file_argument(&args)?;

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

            Ok(Box::new(app::ThothApp::new(settings, file_to_open)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_no_arguments() {
        let args = vec!["thoth".to_string()];
        let result = parse_file_argument(&args).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_valid_file() {
        // Create a temporary test file
        let test_file = std::env::temp_dir().join("test_parse.json");
        std::fs::write(&test_file, r#"{"test": true}"#).unwrap();

        let args = vec!["thoth".to_string(), test_file.to_string_lossy().to_string()];
        let result = parse_file_argument(&args).unwrap();
        assert!(result.is_some());

        let path = result.unwrap();
        assert!(path.exists());
        assert!(path.is_file());

        // Cleanup
        std::fs::remove_file(&test_file).ok();
    }

    #[test]
    fn test_parse_nonexistent_file() {
        let args = vec!["thoth".to_string(), "/nonexistent/file.json".to_string()];
        let result = parse_file_argument(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_directory_not_file() {
        let temp_dir = std::env::temp_dir();
        let args = vec!["thoth".to_string(), temp_dir.to_string_lossy().to_string()];
        let result = parse_file_argument(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_relative_path() {
        // Create a test file in temp
        let test_file = std::env::temp_dir().join("test_relative.json");
        std::fs::write(&test_file, r#"{"test": true}"#).unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(std::env::temp_dir()).unwrap();

        let args = vec!["thoth".to_string(), "test_relative.json".to_string()];
        let result = parse_file_argument(&args).unwrap();
        assert!(result.is_some());

        let path = result.unwrap();
        assert!(path.is_absolute());

        // Restore directory and cleanup
        std::env::set_current_dir(original_dir).unwrap();
        std::fs::remove_file(&test_file).ok();
    }

    #[test]
    fn test_parse_json_extensions() {
        let extensions = vec!["json", "ndjson", "jsonl", "geojson"];

        for ext in extensions {
            let test_file = std::env::temp_dir().join(format!("test.{}", ext));
            std::fs::write(&test_file, r#"{"test": true}"#).unwrap();

            let args = vec!["thoth".to_string(), test_file.to_string_lossy().to_string()];
            let result = parse_file_argument(&args).unwrap();
            assert!(result.is_some(), "Failed for extension: {}", ext);

            std::fs::remove_file(&test_file).ok();
        }
    }
}
