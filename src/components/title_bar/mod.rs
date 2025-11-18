mod linux;
/// Platform-specific title bar implementations for Thoth
///
/// This module provides custom title bars that match each platform's native look:
/// - macOS: Traffic light buttons (red, yellow, green) on the left
/// - Windows: Standard controls (minimize, maximize, close) on the right
/// - Linux: Standard controls on the right (similar to Windows)
mod macos;
mod windows;

use eframe::egui;

/// Props for the title bar component
pub struct TitleBarProps<'a> {
    /// Window title (application name + file name)
    pub title: &'a str,

    /// Current dark mode state
    pub dark_mode: bool,
}

/// Render the platform-specific title bar
pub fn render(ui: &mut egui::Ui, props: TitleBarProps<'_>) {
    #[cfg(target_os = "macos")]
    macos::render_title_bar(ui, props);

    #[cfg(target_os = "windows")]
    windows::render_title_bar(ui, props);

    #[cfg(target_os = "linux")]
    linux::render_title_bar(ui, props);
}

/// Title bar height constant (32px matches VS Code)
pub const TITLE_BAR_HEIGHT: f32 = 32.0;

/// Get the title bar background color based on theme
pub fn title_bar_background(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(0x2d, 0x2d, 0x30) // VS Code dark title bar
    } else {
        egui::Color32::from_rgb(0xdd, 0xdd, 0xdd) // VS Code light title bar
    }
}

/// Get the title bar text color based on theme
pub fn title_bar_text_color(dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        egui::Color32::from_rgb(0xcc, 0xcc, 0xcc)
    } else {
        egui::Color32::from_rgb(0x33, 0x33, 0x33)
    }
}
