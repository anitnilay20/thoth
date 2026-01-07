// Settings dialog components module
//
// This module contains the settings dialog and all its sub-components:
// - Main SettingsDialog (context component with panels and navigation)
// - General settings tab
// - Appearance settings tab
// - Performance settings tab
// - Viewer settings tab
// - Shortcuts settings tab
// - Updates settings tab
// - Advanced settings tab

mod advanced;
mod appearance;
mod general;
mod performance;
mod shortcuts;
mod updates;
mod viewer;

#[cfg(test)]
mod tests;

pub use advanced::AdvancedTab;
pub use appearance::AppearanceTab;
pub use general::GeneralTab;
pub use performance::PerformanceTab;
pub use shortcuts::ShortcutsTab;
pub use updates::UpdatesTab;
pub use viewer::ViewerTab;

use crate::components::traits::ContextComponent;
use crate::settings::Settings;
use crate::theme::{self, ThemeColors};
use eframe::egui;
use std::sync::{Arc, Mutex};

/// Settings dialog with modern UI
pub struct SettingsDialog {
    /// Whether the dialog is open
    pub open: bool,

    /// Currently selected tab
    selected_tab: SettingsTab,

    /// Current settings being edited (not saved until Apply)
    draft_settings: Settings,

    /// Shared state for viewport communication
    viewport_result: Arc<Mutex<Option<Settings>>>,

    /// Shared draft settings for live preview (updated by viewport)
    viewport_draft: Arc<Mutex<Settings>>,

    /// Flag to indicate viewport was closed/cancelled
    viewport_closed: Arc<Mutex<bool>>,

    /// Shared selected tab for viewport
    viewport_selected_tab: Arc<Mutex<SettingsTab>>,

    /// Shared events collected from the dialog
    viewport_events: Arc<Mutex<Vec<SettingsDialogEvent>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Appearance,
    Performance,
    Viewer,
    Shortcuts,
    Updates,
    Advanced,
}

impl SettingsTab {
    fn label(&self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::Appearance => "Appearance",
            SettingsTab::Performance => "Performance",
            SettingsTab::Viewer => "Viewer",
            SettingsTab::Shortcuts => "Shortcuts",
            SettingsTab::Updates => "Updates",
            SettingsTab::Advanced => "Advanced",
        }
    }

    fn icon(&self) -> &'static str {
        match self {
            SettingsTab::General => egui_phosphor::regular::GEAR,
            SettingsTab::Appearance => egui_phosphor::regular::PAINT_BRUSH,
            SettingsTab::Performance => egui_phosphor::regular::GAUGE,
            SettingsTab::Viewer => egui_phosphor::regular::EYE,
            SettingsTab::Shortcuts => egui_phosphor::regular::KEYBOARD,
            SettingsTab::Updates => egui_phosphor::regular::ARROWS_CLOCKWISE,
            SettingsTab::Advanced => egui_phosphor::regular::WRENCH,
        }
    }

    fn all() -> &'static [SettingsTab] {
        &[
            SettingsTab::General,
            SettingsTab::Appearance,
            SettingsTab::Performance,
            SettingsTab::Viewer,
            SettingsTab::Shortcuts,
            SettingsTab::Updates,
            SettingsTab::Advanced,
        ]
    }
}

impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            selected_tab: SettingsTab::General,
            draft_settings: Settings::default(),
            viewport_result: Arc::new(Mutex::new(None)),
            viewport_draft: Arc::new(Mutex::new(Settings::default())),
            viewport_closed: Arc::new(Mutex::new(false)),
            viewport_selected_tab: Arc::new(Mutex::new(SettingsTab::General)),
            viewport_events: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl SettingsDialog {
    /// Open the settings dialog with current settings
    pub fn open(&mut self, current_settings: &Settings) {
        self.open_with_tab(current_settings, None);
    }

    /// Open the settings dialog on the Updates tab
    pub fn open_updates(&mut self, current_settings: &Settings) {
        self.open_with_tab(current_settings, Some(SettingsTab::Updates));
    }

    /// Open the settings dialog with a specific tab selected
    fn open_with_tab(&mut self, current_settings: &Settings, tab: Option<SettingsTab>) {
        self.open = true;
        self.draft_settings = current_settings.clone();

        // Set the selected tab if specified
        if let Some(tab) = tab {
            self.selected_tab = tab;
        }

        // Update viewport_draft with current settings
        if let Ok(mut draft) = self.viewport_draft.lock() {
            *draft = current_settings.clone();
        }

        // Reset closed flag
        if let Ok(mut closed) = self.viewport_closed.lock() {
            *closed = false;
        }

        // Update selected tab
        if let Ok(mut viewport_tab) = self.viewport_selected_tab.lock() {
            *viewport_tab = self.selected_tab;
        }

        // Clear any previous events
        if let Ok(mut events) = self.viewport_events.lock() {
            events.clear();
        }
    }

    /// Helper method to render tab content with proper event handling
    /// This consolidates the duplicate tab rendering logic
    fn render_tab_content(
        ui: &mut egui::Ui,
        tab: SettingsTab,
        settings: &mut Settings,
        theme_colors: &ThemeColors,
        update_state: Option<&crate::update::UpdateState>,
        current_version: &str,
        dialog_events: &mut Vec<SettingsDialogEvent>,
    ) {
        use crate::components::traits::StatelessComponent;

        match tab {
            SettingsTab::General => {
                let output = GeneralTab::render(
                    ui,
                    general::GeneralTabProps {
                        window_settings: &settings.window,
                        ui_settings: &settings.ui,
                    },
                );

                // Handle events
                for event in output.events {
                    use general::GeneralTabEvent;
                    match event {
                        GeneralTabEvent::WindowWidthChanged(width) => {
                            settings.window.default_width = width;
                        }
                        GeneralTabEvent::WindowHeightChanged(height) => {
                            settings.window.default_height = height;
                        }
                        GeneralTabEvent::RememberSidebarStateChanged(value) => {
                            settings.ui.remember_sidebar_state = value;
                        }
                        GeneralTabEvent::ShowToolbarChanged(value) => {
                            settings.ui.show_toolbar = value;
                        }
                        GeneralTabEvent::ShowStatusBarChanged(value) => {
                            settings.ui.show_status_bar = value;
                        }
                        GeneralTabEvent::EnableAnimationsChanged(value) => {
                            settings.ui.enable_animations = value;
                        }
                        GeneralTabEvent::SidebarWidthChanged(width) => {
                            settings.ui.sidebar_width = width;
                        }
                    }
                }
            }
            SettingsTab::Appearance => {
                AppearanceTab::render(ui, settings, theme_colors);
            }
            SettingsTab::Performance => {
                let output = PerformanceTab::render(
                    ui,
                    performance::PerformanceTabProps {
                        performance_settings: &settings.performance,
                        theme_colors,
                    },
                );

                // Handle events
                for event in output.events {
                    use performance::PerformanceTabEvent;
                    match event {
                        PerformanceTabEvent::CacheSizeChanged(size) => {
                            settings.performance.cache_size = size;
                        }
                        PerformanceTabEvent::MaxRecentFilesChanged(max) => {
                            settings.performance.max_recent_files = max;
                        }
                        PerformanceTabEvent::NavigationHistorySizeChanged(size) => {
                            settings.performance.navigation_history_size = size;
                        }
                    }
                }
            }
            SettingsTab::Viewer => {
                let output = ViewerTab::render(
                    ui,
                    viewer::ViewerTabProps {
                        viewer_settings: &settings.viewer,
                        theme_colors,
                    },
                );

                // Handle events
                for event in output.events {
                    use viewer::ViewerTabEvent;
                    match event {
                        ViewerTabEvent::SyntaxHighlightingChanged(enabled) => {
                            settings.viewer.syntax_highlighting = enabled;
                        }
                    }
                }
            }
            SettingsTab::Shortcuts => {
                let _output = ShortcutsTab::render(
                    ui,
                    shortcuts::ShortcutsTabProps {
                        shortcuts: &settings.shortcuts,
                        theme_colors,
                    },
                );
                // No events to handle yet - shortcuts are read-only
            }
            SettingsTab::Updates => {
                let output = UpdatesTab::render(
                    ui,
                    updates::UpdatesTabProps {
                        update_settings: &settings.updates,
                        update_state,
                        current_version,
                        theme_colors,
                    },
                );

                // Handle events
                for event in output.events {
                    use updates::UpdatesTabEvent;
                    match event {
                        UpdatesTabEvent::AutoCheckChanged(value) => {
                            settings.updates.auto_check = value;
                        }
                        UpdatesTabEvent::CheckIntervalChanged(hours) => {
                            settings.updates.check_interval_hours = hours;
                        }
                        UpdatesTabEvent::CheckForUpdates => {
                            dialog_events.push(SettingsDialogEvent::CheckForUpdates);
                        }
                        UpdatesTabEvent::DownloadUpdate => {
                            dialog_events.push(SettingsDialogEvent::DownloadUpdate);
                        }
                        UpdatesTabEvent::InstallUpdate => {
                            dialog_events.push(SettingsDialogEvent::InstallUpdate);
                        }
                    }
                }
            }
            SettingsTab::Advanced => {
                // Check if thoth is in PATH
                let is_in_path = crate::path_registry::is_in_path();

                let output = AdvancedTab::render(
                    ui,
                    advanced::AdvancedTabProps {
                        dev_settings: &settings.dev,
                        theme_colors,
                        is_in_path,
                    },
                );

                // Handle events
                for event in output.events {
                    use advanced::AdvancedTabEvent;
                    match event {
                        AdvancedTabEvent::ShowProfilerChanged(enabled) => {
                            settings.dev.show_profiler = enabled;
                        }
                        AdvancedTabEvent::RegisterInPath => {
                            dialog_events.push(SettingsDialogEvent::RegisterInPath);
                        }
                        AdvancedTabEvent::UnregisterFromPath => {
                            dialog_events.push(SettingsDialogEvent::UnregisterFromPath);
                        }
                    }
                }
            }
        }
    }
}

/// Props for SettingsDialog when used as a ContextComponent
pub struct SettingsDialogProps<'a> {
    /// Current update state (optional - for Updates tab)
    pub update_state: Option<&'a crate::update::UpdateState>,
    /// Current version string
    pub current_version: &'a str,
}

/// Events from SettingsDialog that need to be handled by the application
#[derive(Debug, Clone)]
pub enum SettingsDialogEvent {
    CheckForUpdates,
    DownloadUpdate,
    InstallUpdate,
    RegisterInPath,
    UnregisterFromPath,
}

/// Output from SettingsDialog
pub struct SettingsDialogOutput {
    /// New settings if Apply was clicked
    pub new_settings: Option<Settings>,
    /// Events that need to be handled by the application
    pub events: Vec<SettingsDialogEvent>,
}

impl ContextComponent for SettingsDialog {
    type Props<'a> = SettingsDialogProps<'a>;
    type Output = SettingsDialogOutput;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
        // If not open, return early
        if !self.open {
            return SettingsDialogOutput {
                new_settings: None,
                events: Vec::new(),
            };
        }

        // Use viewport mode (separate OS window) for settings
        let viewport_id = egui::ViewportId::from_hash_of("thoth_settings");

        // Clone Arc for use in the closure
        let viewport_result = Arc::clone(&self.viewport_result);
        let viewport_closed = Arc::clone(&self.viewport_closed);
        let draft_settings = Arc::clone(&self.viewport_draft);
        let selected_tab = Arc::clone(&self.viewport_selected_tab);
        let viewport_events = Arc::clone(&self.viewport_events);

        // Clone update state and version for the viewport
        let update_state_clone = props.update_state.cloned();
        let current_version = props.current_version.to_string();

        ctx.show_viewport_deferred(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title("Thoth - Settings")
                .with_inner_size([900.0, 600.0])
                .with_min_inner_size([800.0, 500.0]),
            move |ctx, class| {
                // Check if viewport is being closed (X button clicked)
                if class == egui::ViewportClass::Deferred
                    && ctx.input(|i| i.viewport().close_requested())
                {
                    if let Ok(mut closed) = viewport_closed.lock() {
                        *closed = true;
                    }
                    return;
                }

                // Apply theme from draft settings so changes preview in real-time
                if let Ok(settings) = draft_settings.lock() {
                    theme::apply_theme(ctx, &settings);
                }

                // Get theme colors
                let theme_colors = ctx.memory(|mem| {
                    mem.data
                        .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                        .unwrap_or_else(|| {
                            theme::Theme::for_dark_mode(ctx.style().visuals.dark_mode).colors()
                        })
                });

                let mut new_settings = None;

                // Top panel with title and buttons
                egui::TopBottomPanel::top("settings_top")
                    .frame(
                        egui::Frame::default()
                            .fill(theme_colors.crust)
                            .inner_margin(egui::Margin::symmetric(16, 12)),
                    )
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Edit settings.toml button
                                    let btn = ui.button(
                                        egui::RichText::new("Edit settings in settings.toml")
                                            .size(13.0),
                                    );
                                    if btn.hovered() {
                                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if btn.clicked() {
                                        if let Ok(path) = Settings::settings_file_path() {
                                            let _ = open::that(path);
                                        }
                                    }
                                },
                            );
                        });
                    });

                // Bottom panel with Cancel/Apply buttons
                egui::TopBottomPanel::bottom("settings_bottom")
                    .frame(
                        egui::Frame::default()
                            .fill(theme_colors.crust)
                            .inner_margin(egui::Margin::symmetric(16, 12)),
                    )
                    .show(ctx, |ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let apply_btn = ui.button(egui::RichText::new("Apply").size(14.0));
                            if apply_btn.hovered() {
                                ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if apply_btn.clicked() {
                                if let Ok(settings) = draft_settings.lock() {
                                    new_settings = Some(settings.clone());
                                }
                            }

                            ui.add_space(8.0);

                            let cancel_btn = ui.button(egui::RichText::new("Cancel").size(14.0));
                            if cancel_btn.hovered() {
                                ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if cancel_btn.clicked() {
                                if let Ok(mut closed) = viewport_closed.lock() {
                                    *closed = true;
                                }
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        });
                    });

                // Left sidebar with icons
                egui::SidePanel::left("settings_sidebar")
                    .resizable(false)
                    .exact_width(200.0)
                    .frame(
                        egui::Frame::default()
                            .fill(theme_colors.mantle)
                            .inner_margin(12.0),
                    )
                    .show(ctx, |ui| {
                        ui.add_space(16.0);

                        // Render navigation tabs with icons
                        for tab in SettingsTab::all() {
                            let is_selected = if let Ok(current_tab) = selected_tab.lock() {
                                *current_tab == *tab
                            } else {
                                false
                            };

                            let bg_color = if is_selected {
                                theme_colors.surface1
                            } else {
                                egui::Color32::TRANSPARENT
                            };

                            let hover_color = if !is_selected {
                                theme_colors.surface0
                            } else {
                                theme_colors.surface1
                            };

                            ui.vertical(|ui| {
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), 56.0),
                                    egui::Sense::click(),
                                );

                                // Draw background
                                let bg = if response.hovered() {
                                    hover_color
                                } else {
                                    bg_color
                                };

                                ui.painter().rect_filled(rect, 4.0, bg);

                                // Draw icon and label
                                let icon_pos = rect.center_top() + egui::vec2(0.0, 12.0);
                                ui.painter().text(
                                    icon_pos,
                                    egui::Align2::CENTER_TOP,
                                    tab.icon(),
                                    egui::FontId::proportional(20.0),
                                    theme_colors.text,
                                );

                                let label_pos = icon_pos + egui::vec2(0.0, 24.0);
                                ui.painter().text(
                                    label_pos,
                                    egui::Align2::CENTER_TOP,
                                    tab.label(),
                                    egui::FontId::proportional(13.0),
                                    theme_colors.text,
                                );

                                if response.hovered() {
                                    ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                                }

                                if response.clicked() {
                                    if let Ok(mut current_tab) = selected_tab.lock() {
                                        *current_tab = *tab;
                                    }
                                }
                            });

                            ui.add_space(8.0);
                        }
                    });

                // Central content area
                egui::CentralPanel::default()
                    .frame(egui::Frame::default().fill(theme_colors.base))
                    .show(ctx, |ui| {
                        if let (Ok(current_tab), Ok(mut settings), Ok(mut events)) = (
                            selected_tab.lock(),
                            draft_settings.lock(),
                            viewport_events.lock(),
                        ) {
                            Self::render_tab_content(
                                ui,
                                *current_tab,
                                &mut settings,
                                &theme_colors,
                                update_state_clone.as_ref(),
                                &current_version,
                                &mut events,
                            );
                        }
                    });

                // If Apply was clicked, store result and close viewport
                if let Some(settings) = new_settings {
                    if let Ok(mut result) = viewport_result.lock() {
                        *result = Some(settings);
                    }
                    if let Ok(mut closed) = viewport_closed.lock() {
                        *closed = true;
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            },
        );

        // Check if viewport was closed or Apply was clicked
        let mut result = None;
        let mut collected_events = Vec::new();

        if let Ok(mut closed) = self.viewport_closed.lock() {
            if *closed {
                self.open = false;
                *closed = false; // Reset for next time

                // Check if Apply was clicked (result will be Some)
                if let Ok(mut viewport_result) = self.viewport_result.lock() {
                    result = viewport_result.take();
                }
            }
        }

        // Collect any events that were generated
        if let Ok(mut events) = self.viewport_events.lock() {
            collected_events = events.drain(..).collect();
        }

        SettingsDialogOutput {
            new_settings: result,
            events: collected_events,
        }
    }
}
