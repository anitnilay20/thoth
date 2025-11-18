mod linux;
/// Platform-specific title bar implementations for Thoth
///
/// This module provides custom title bars that match each platform's native look:
/// - macOS: Traffic light buttons (red, yellow, green) on the left
/// - Windows: Standard controls (minimize, maximize, close) on the right
/// - Linux: Standard controls on the right (similar to Windows)
mod macos;
mod windows;

use crate::components::traits::ContextComponent;
use eframe::egui;

/// Title bar component with platform-specific rendering
#[derive(Default)]
pub struct TitleBar;

/// Props for the title bar component (immutable, one-way binding)
pub struct TitleBarProps<'a> {
    /// Window title (application name + file name)
    pub title: &'a str,

    /// Current dark mode state
    pub dark_mode: bool,
}

/// Events emitted by the title bar (bottom-to-top communication)
#[derive(Debug, Clone)]
pub enum TitleBarEvent {
    /// Window close requested
    Close,
    /// Window minimize requested
    Minimize,
    /// Window maximize/restore requested
    Maximize,
}

/// Output from title bar component
pub struct TitleBarOutput {
    /// List of events that occurred during this frame
    pub events: Vec<TitleBarEvent>,
}

impl ContextComponent for TitleBar {
    type Props<'a> = TitleBarProps<'a>;
    type Output = TitleBarOutput;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();

        egui::TopBottomPanel::top("title_bar")
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                #[cfg(target_os = "macos")]
                macos::render_title_bar(ui, props, &mut events);

                #[cfg(target_os = "windows")]
                windows::render_title_bar(ui, props, &mut events);

                #[cfg(target_os = "linux")]
                linux::render_title_bar(ui, props, &mut events);
            });

        TitleBarOutput { events }
    }
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
