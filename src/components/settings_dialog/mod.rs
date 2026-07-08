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
mod general;
mod helpers;
mod interface;
mod performance;
mod plugins;
mod shortcuts;
mod theme_picker;
mod updates;
mod viewer;

#[cfg(test)]
mod tests;

pub use advanced::AdvancedTab;
pub use general::GeneralTab;
pub use performance::PerformanceTab;
pub use shortcuts::ShortcutsTab;
pub use updates::UpdatesTab;
pub use viewer::ViewerTab;

use crate::components::settings_dialog::plugins::{PluginsTab, PluginsTabEvent, PluginsTabProps};
use crate::components::traits::ContextComponent;
use crate::notification::{Notification, NotificationManager, NotificationStatus};
use crate::settings::Settings;
use crate::theme::{self, Theme, ThemeColors, icon_rich_text, phosphor_font_id};
use eframe::egui;
use std::sync::{Arc, Mutex};
use thoth_plugin_sdk::components::{Button, ButtonColor, ButtonType, IconButton, Input};

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

    /// Original settings at dialog-open time — used as dirty baseline.
    /// Reset still goes to Settings::default(), not this.
    viewport_baseline: Arc<Mutex<Settings>>,

    /// Flag to indicate viewport was closed/cancelled
    viewport_closed: Arc<Mutex<bool>>,

    /// Shared selected tab for viewport
    viewport_selected_tab: Arc<Mutex<SettingsTab>>,

    /// Shared events collected from the dialog
    viewport_events: Arc<Mutex<Vec<SettingsDialogEvent>>>,

    /// Plugin Id for selected plugin settings (shared into viewport closure)
    open_plugin_settings_id: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsTab {
    General,
    Interface,
    Viewer,
    Performance,
    Shortcuts,
    Plugins,
    Updates,
    Developer,
}

impl SettingsTab {
    fn label(self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::Interface => "Interface",
            SettingsTab::Viewer => "Viewer",
            SettingsTab::Performance => "Performance",
            SettingsTab::Shortcuts => "Shortcuts",
            SettingsTab::Plugins => "Plugins",
            SettingsTab::Updates => "Updates",
            SettingsTab::Developer => "Developer",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            SettingsTab::General => "Theme, typography, window defaults",
            SettingsTab::Interface => "Sidebar, toolbar, status bar, animations",
            SettingsTab::Viewer => "Syntax highlighting and display",
            SettingsTab::Performance => "Cache, history and recent files",
            SettingsTab::Shortcuts => "Keyboard shortcuts per action",
            SettingsTab::Plugins => "Installed plugins and network policies",
            SettingsTab::Updates => "Auto-update and version info",
            SettingsTab::Developer => "Profiler and configuration file",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            SettingsTab::General => egui_phosphor::regular::SLIDERS,
            SettingsTab::Interface => egui_phosphor::regular::SIDEBAR,
            SettingsTab::Viewer => egui_phosphor::regular::EYE,
            SettingsTab::Performance => egui_phosphor::regular::GAUGE,
            SettingsTab::Shortcuts => egui_phosphor::regular::KEYBOARD,
            SettingsTab::Plugins => egui_phosphor::regular::PLUGS,
            SettingsTab::Updates => egui_phosphor::regular::ARROWS_CLOCKWISE,
            SettingsTab::Developer => egui_phosphor::regular::WRENCH,
        }
    }

    fn all() -> &'static [SettingsTab] {
        &[
            SettingsTab::General,
            SettingsTab::Interface,
            SettingsTab::Viewer,
            SettingsTab::Performance,
            SettingsTab::Shortcuts,
            SettingsTab::Plugins,
            SettingsTab::Updates,
            SettingsTab::Developer,
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
            viewport_baseline: Arc::new(Mutex::new(Settings::default())),
            viewport_closed: Arc::new(Mutex::new(false)),
            viewport_selected_tab: Arc::new(Mutex::new(SettingsTab::General)),
            viewport_events: Arc::new(Mutex::new(Vec::new())),
            open_plugin_settings_id: Arc::new(Mutex::new(None)),
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

        // Update viewport_draft and baseline with current settings
        if let Ok(mut draft) = self.viewport_draft.lock() {
            *draft = current_settings.clone();
        }
        if let Ok(mut baseline) = self.viewport_baseline.lock() {
            *baseline = current_settings.clone();
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
    #[allow(clippy::too_many_arguments)]
    fn render_tab_content(
        ui: &mut egui::Ui,
        tab: SettingsTab,
        settings: &mut Settings,
        baseline: &Settings,
        theme_colors: &ThemeColors,
        update_state: Option<&crate::update::UpdateState>,
        last_check: Option<chrono::DateTime<chrono::Utc>>,
        current_version: &str,
        dialog_events: &mut Vec<SettingsDialogEvent>,
        open_plugin_settings_id: &Arc<Mutex<Option<String>>>,
    ) {
        use crate::components::traits::StatelessComponent;

        match tab {
            SettingsTab::General => {
                let output = GeneralTab::render(
                    ui,
                    general::GeneralTabProps {
                        settings,
                        baseline,
                        theme_colors,
                    },
                );
                for event in output.events {
                    use general::GeneralTabEvent;
                    match event {
                        GeneralTabEvent::ThemeName(name) => {
                            settings.theme = Theme::from_name(&name);
                        }
                        GeneralTabEvent::FontSize(s) => {
                            settings.font_size = s;
                        }
                        GeneralTabEvent::FontFamily(f) => {
                            settings.font_family = f;
                        }
                        GeneralTabEvent::WindowWidth(w) => {
                            settings.window.default_width = w;
                        }
                        GeneralTabEvent::WindowHeight(h) => {
                            settings.window.default_height = h;
                        }
                    }
                }
            }
            SettingsTab::Interface => {
                let output = interface::InterfaceTab::render(
                    ui,
                    interface::InterfaceTabProps {
                        ui_settings: &settings.ui,
                        baseline: &baseline.ui,
                        theme_colors,
                    },
                );
                for event in output.events {
                    use interface::InterfaceTabEvent;
                    match event {
                        InterfaceTabEvent::SidebarWidthChanged(w) => {
                            settings.ui.sidebar_width = w;
                        }
                        InterfaceTabEvent::RememberSidebarStateChanged(v) => {
                            settings.ui.remember_sidebar_state = v;
                        }
                        InterfaceTabEvent::ShowToolbarChanged(v) => {
                            settings.ui.show_toolbar = v;
                        }
                        InterfaceTabEvent::ShowStatusBarChanged(v) => {
                            settings.ui.show_status_bar = v;
                        }
                        InterfaceTabEvent::EnableAnimationsChanged(v) => {
                            settings.ui.enable_animations = v;
                        }
                    }
                }
            }
            SettingsTab::Developer => {
                let is_in_path = crate::platform::path_registry::is_in_path();
                let output = AdvancedTab::render(
                    ui,
                    advanced::AdvancedTabProps {
                        dev_settings: &settings.dev,
                        theme_colors,
                        is_in_path,
                    },
                );
                for event in output.events {
                    use advanced::AdvancedTabEvent;
                    match event {
                        AdvancedTabEvent::ShowProfilerChanged(v) => {
                            settings.dev.show_profiler = v;
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
            SettingsTab::Plugins => {
                // let current_ui = plugin_settings_ui.lock().ok().and_then(|g| g.clone());

                let output = PluginsTab::render(
                    ui,
                    PluginsTabProps {
                        plugin_settings: settings.plugins.clone(),
                        active_plugin_settings: open_plugin_settings_id
                            .lock()
                            .ok()
                            .and_then(|g| g.clone()),
                    },
                );

                for event in output.events {
                    match event {
                        PluginsTabEvent::EnablePlugins(enabled) => {
                            settings.plugins.enabled = enabled;
                        }
                        PluginsTabEvent::TogglePlugin { id, enabled } => {
                            if enabled {
                                settings.plugins.disabled_plugin_ids.retain(|x| x != &id);
                            } else if !settings.plugins.disabled_plugin_ids.contains(&id) {
                                settings.plugins.disabled_plugin_ids.push(id);
                            }
                        }
                        PluginsTabEvent::UninstallPlugin(id) => {
                            if let Some(Some(pm)) = crate::PLUGIN_MANAGER.get() {
                                let wasm_path =
                                    pm.registry.get_by_id(&id).and_then(|p| p.location.clone());
                                if let Some(location) = wasm_path {
                                    let path = std::path::Path::new(&location);
                                    if let Some(dir) = path.parent()
                                        && dir.exists()
                                        && let Err(e) = std::fs::remove_dir_all(dir)
                                    {
                                        eprintln!(
                                            "Failed to remove plugin directory {}: {e}",
                                            dir.display()
                                        );
                                    }
                                }
                                settings.plugins.disabled_plugin_ids.retain(|x| x != &id);
                            }
                        }
                        // TODO: more specific events for different setting types so the UI doesn't have to re-render the entire settings page on every change
                        // PluginsTabEvent::PluginUpdateSetting(plugin_id, plugin_settings) => {
                        //     settings
                        //         .plugins
                        //         .plugin_settings
                        //         .insert(plugin_id, plugin_settings);
                        // }
                        PluginsTabEvent::OpenSettingsForPlugin(plugin_id) => {
                            if let Ok(mut id) = open_plugin_settings_id.lock() {
                                *id = plugin_id;
                            }
                        }
                        PluginsTabEvent::UpdateNetworkPolicy { plugin_id, policy } => {
                            settings.plugins.network_policies.insert(plugin_id, policy);
                        }
                    }
                }
            }
            SettingsTab::Updates => {
                let output = UpdatesTab::render(
                    ui,
                    updates::UpdatesTabProps {
                        update_settings: &settings.updates,
                        update_state,
                        last_check,
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
            } // Developer tab is handled inline above via AdvancedTab
        }
    }
}

/// Props for SettingsDialog when used as a ContextComponent
pub struct SettingsDialogProps<'a> {
    /// Current update state (optional - for Updates tab)
    pub update_state: Option<&'a crate::update::UpdateState>,
    /// Timestamp of the last update check
    pub last_check: Option<chrono::DateTime<chrono::Utc>>,
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

/// Returns true when a section's fields differ from the baseline.
fn section_is_dirty(tab: SettingsTab, draft: &Settings, baseline: &Settings) -> bool {
    match tab {
        SettingsTab::General => {
            draft.theme != baseline.theme
                || draft.font_size != baseline.font_size
                || draft.font_family != baseline.font_family
                || draft.window.default_width != baseline.window.default_width
                || draft.window.default_height != baseline.window.default_height
        }
        SettingsTab::Interface => {
            draft.ui.sidebar_width != baseline.ui.sidebar_width
                || draft.ui.show_toolbar != baseline.ui.show_toolbar
                || draft.ui.show_status_bar != baseline.ui.show_status_bar
                || draft.ui.enable_animations != baseline.ui.enable_animations
                || draft.ui.remember_sidebar_state != baseline.ui.remember_sidebar_state
        }
        SettingsTab::Viewer => {
            draft.viewer.syntax_highlighting != baseline.viewer.syntax_highlighting
        }
        SettingsTab::Performance => {
            draft.performance.cache_size != baseline.performance.cache_size
                || draft.performance.max_recent_files != baseline.performance.max_recent_files
                || draft.performance.navigation_history_size
                    != baseline.performance.navigation_history_size
        }
        SettingsTab::Shortcuts => false,
        SettingsTab::Plugins => {
            draft.plugins.enabled != baseline.plugins.enabled
                || draft.plugins.disabled_plugin_ids != baseline.plugins.disabled_plugin_ids
                || draft.plugins.network_policies != baseline.plugins.network_policies
                || draft.plugins.plugin_settings != baseline.plugins.plugin_settings
        }
        SettingsTab::Updates => {
            draft.updates.auto_check != baseline.updates.auto_check
                || draft.updates.check_interval_hours != baseline.updates.check_interval_hours
        }
        SettingsTab::Developer => draft.dev.show_profiler != baseline.dev.show_profiler,
    }
}

/// Reset a section's fields in `draft` back to defaults.
fn reset_section(tab: SettingsTab, draft: &mut Settings) {
    let def = Settings::default();
    match tab {
        SettingsTab::General => {
            draft.theme = def.theme;
            draft.font_size = def.font_size;
            draft.font_family = def.font_family;
            draft.window = def.window;
        }
        SettingsTab::Interface => {
            draft.ui = def.ui;
        }
        SettingsTab::Viewer => {
            draft.viewer = def.viewer;
        }
        SettingsTab::Performance => {
            draft.performance = def.performance;
        }
        SettingsTab::Updates => {
            draft.updates = def.updates;
        }
        SettingsTab::Developer => {
            draft.dev = def.dev;
        }
        SettingsTab::Plugins => {
            draft.plugins.disabled_plugin_ids = def.plugins.disabled_plugin_ids;
            draft.plugins.plugin_settings = def.plugins.plugin_settings;
        }
        _ => {}
    }
}

impl ContextComponent for SettingsDialog {
    type Props<'a> = SettingsDialogProps<'a>;
    type Output = SettingsDialogOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
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
        let open_plugin_settings_id = Arc::clone(&self.open_plugin_settings_id);
        let viewport_baseline = Arc::clone(&self.viewport_baseline);

        // Clone update state and version for the viewport
        let update_state_clone = props.update_state.cloned();
        let last_check_clone = props.last_check;
        let current_version = props.current_version.to_string();

        // Size the settings window to 75% of the parent window, clamped to a
        // sensible minimum so the layout never breaks on small screens.
        let parent_size = ui.ctx().content_rect().size();
        let settings_w = (parent_size.x * 0.85).max(800.0);
        let settings_h = (parent_size.y * 0.85).max(520.0);

        ui.ctx().show_viewport_deferred(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title("Thoth - Settings")
                .with_decorations(false)
                .with_inner_size([settings_w, settings_h])
                .with_min_inner_size([800.0, 520.0]),
            move |ui, class| {
                let ctx = ui.ctx().clone();

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
                    theme::apply_theme(&ctx, &settings);
                }

                // Get theme colors
                let theme_colors = ctx.memory(|mem| {
                    mem.data
                        .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                        .unwrap_or_else(|| {
                            theme::Theme::for_dark_mode(ctx.global_style().visuals.dark_mode)
                                .colors()
                        })
                });

                let mut new_settings = None;

                // ── Custom title bar (32px) ───────────────────────────────
                egui::Panel::top("settings_titlebar")
                    .exact_size(32.0)
                    .frame(
                        egui::Frame::default()
                            .fill(theme_colors.bg_sunken)
                            .inner_margin(egui::Margin::symmetric(12, 0)),
                    )
                    .show_inside(ui, |ui| {
                        // Make the whole bar draggable so the window can be moved
                        let drag_resp = ui.interact(
                            ui.available_rect_before_wrap(),
                            ui.id().with("titlebar_drag"),
                            egui::Sense::click_and_drag(),
                        );
                        if drag_resp.dragged() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }

                        ui.horizontal_centered(|ui| {
                            // App icon glyph
                            ui.label(
                                icon_rich_text(egui_phosphor::regular::TREE_STRUCTURE, 13.0)
                                    .color(theme_colors.accent),
                            );
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("Settings")
                                    .size(13.0)
                                    .color(theme_colors.fg),
                            );

                            // Close button (right-aligned)
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let close_out = ui.add(
                                        IconButton::builder()
                                            .icon(egui_phosphor::regular::X)
                                            .tooltip("Close")
                                            .frame(false)
                                            .size_px(20.0)
                                            .build(),
                                    );
                                    if close_out.clicked() {
                                        if let Ok(mut closed) = viewport_closed.lock() {
                                            *closed = true;
                                        }
                                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    }
                                },
                            );
                        });

                        // Bottom divider
                        ui.painter().hline(
                            ui.clip_rect().x_range(),
                            ui.clip_rect().bottom(),
                            egui::Stroke::new(1.0, theme_colors.surface),
                        );
                    });

                // ── Footer (56px) ────────────────────────────────────────
                egui::Panel::bottom("settings_bottom")
                    .exact_size(56.0)
                    .frame(
                        egui::Frame::default()
                            .fill(theme_colors.bg_sunken)
                            .inner_margin(egui::Margin::symmetric(16, 0)),
                    )
                    .show_inside(ui, |ui| {
                        // Top divider
                        ui.painter().hline(
                            ui.clip_rect().x_range(),
                            ui.clip_rect().top(),
                            egui::Stroke::new(1.0, theme_colors.surface_raised),
                        );

                        ui.horizontal_centered(|ui| {
                            // Dirty indicator (left side)
                            let (is_dirty, dirty_count) = if let (Ok(draft), Ok(baseline)) =
                                (draft_settings.lock(), viewport_baseline.lock())
                            {
                                let count = SettingsTab::all()
                                    .iter()
                                    .filter(|&&t| section_is_dirty(t, &draft, &baseline))
                                    .count();
                                (count > 0, count)
                            } else {
                                (false, 0)
                            };

                            if is_dirty {
                                ui.painter().circle_filled(
                                    ui.cursor().center_top() + egui::vec2(5.0, 10.0),
                                    4.0,
                                    theme_colors.accent,
                                );
                                ui.add_space(14.0);
                                let label = if dirty_count == 1 {
                                    "1 unsaved change".to_string()
                                } else {
                                    format!("{dirty_count} unsaved changes")
                                };
                                ui.label(
                                    egui::RichText::new(label)
                                        .size(12.0)
                                        .color(theme_colors.fg_muted),
                                );
                            }

                            // Buttons (right side)
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Save button
                                    let save_btn = ui.add(
                                        Button::builder()
                                            .label("Save changes")
                                            .button_type(ButtonType::Elevated)
                                            .color(ButtonColor::Primary)
                                            .size(13.0)
                                            .enabled(is_dirty)
                                            .build(),
                                    );
                                    if save_btn.clicked()
                                        && let Ok(settings) = draft_settings.lock()
                                    {
                                        new_settings = Some(settings.clone());
                                        NotificationManager::notify(
                                            Notification::new("Setting saved.", "")
                                                .with_toast(true)
                                                .with_status(NotificationStatus::Completed),
                                        );
                                    }

                                    ui.add_space(8.0);

                                    // Cancel button
                                    let cancel_btn = ui.add(
                                        Button::builder()
                                            .label("Cancel")
                                            .button_type(ButtonType::Elevated)
                                            .color(ButtonColor::Default)
                                            .size(13.0)
                                            .build(),
                                    );
                                    if cancel_btn.clicked() {
                                        if let Ok(mut closed) = viewport_closed.lock() {
                                            *closed = true;
                                        }
                                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    }

                                    ui.add_space(8.0);

                                    // Reset section button
                                    if is_dirty {
                                        let reset_btn = ui.add(
                                            Button::builder()
                                                .label("Reset section")
                                                .button_type(ButtonType::Text)
                                                .color(ButtonColor::Default)
                                                .size(12.0)
                                                .build(),
                                        );
                                        if reset_btn.clicked()
                                            && let (Ok(mut draft), Ok(tab)) =
                                                (draft_settings.lock(), selected_tab.lock())
                                        {
                                            reset_section(*tab, &mut draft);
                                        }
                                    }
                                },
                            );
                        });
                    });

                // ── Sidebar (240px) ─────────────────────────────────────
                egui::Panel::left("settings_sidebar")
                    .resizable(false)
                    .exact_size(240.0)
                    .frame(
                        egui::Frame::default()
                            .fill(theme_colors.bg_panel)
                            .inner_margin(egui::Margin::ZERO),
                    )
                    .show_inside(ui, |ui| {
                        // Title
                        egui::Frame::new()
                            .inner_margin(egui::Margin {
                                left: 16,
                                right: 16,
                                top: 16,
                                bottom: 8,
                            })
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("Settings")
                                        .size(14.0)
                                        .strong()
                                        .color(theme_colors.fg),
                                );
                            });

                        // Search box
                        let search_id = egui::Id::new("settings_search_query");
                        let mut search_query: String =
                            ctx.data(|d| d.get_temp(search_id).unwrap_or_default());
                        egui::Frame::NONE
                            .outer_margin(egui::Margin::symmetric(12, 4))
                            .show(ui, |ui| {
                                let mut input = Input::builder()
                                    .value(search_query.clone())
                                    .placeholder("Search settings…")
                                    .icon(egui_phosphor::regular::MAGNIFYING_GLASS)
                                    .rows(1)
                                    .build();
                                let r = input.show(ui);
                                if r.inner {
                                    search_query = input.value.clone();
                                }
                            });
                        ctx.data_mut(|d| d.insert_temp(search_id, search_query.clone()));

                        ui.add_space(4.0);
                        ui.painter().hline(
                            ui.clip_rect().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(0.5, theme_colors.surface_raised),
                        );
                        ui.add_space(4.0);

                        // ── Settings file path (sidebar bottom) ─────────
                        egui::Panel::bottom("sidebar_settings_file")
                            .exact_size(36.0)
                            .frame(
                                egui::Frame::default()
                                    .fill(theme_colors.bg_panel)
                                    .inner_margin(egui::Margin::symmetric(12, 0)),
                            )
                            .show_inside(ui, |ui| {
                                ui.painter().hline(
                                    ui.clip_rect().x_range(),
                                    ui.clip_rect().top(),
                                    egui::Stroke::new(0.5, theme_colors.surface_raised),
                                );
                                ui.horizontal_centered(|ui| {
                                    ui.label(
                                        icon_rich_text(egui_phosphor::regular::FILE_TEXT, 11.0)
                                            .color(theme_colors.fg_muted),
                                    );
                                    ui.add_space(4.0);
                                    let path_str = crate::settings::Settings::settings_file_path()
                                        .map(|p| {
                                            p.file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("settings.toml")
                                                .to_string()
                                        })
                                        .unwrap_or_else(|_| "settings.toml".to_string());
                                    let btn = ui.add(
                                        Button::builder()
                                            .label(path_str)
                                            .button_type(ButtonType::Text)
                                            .color(ButtonColor::Default)
                                            .size(11.0)
                                            .build(),
                                    );
                                    if btn.clicked()
                                        && let Ok(path) =
                                            crate::settings::Settings::settings_file_path()
                                    {
                                        let _ = open::that(path);
                                    }
                                    btn.on_hover_text(
                                        crate::settings::Settings::settings_file_path()
                                            .map(|p| p.to_string_lossy().to_string())
                                            .unwrap_or_default(),
                                    );
                                });
                            });

                        // Nav items
                        egui::ScrollArea::vertical()
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                // Compute dirty-ness per section so we can show dots
                                let (current_tab, dirty_sections) =
                                    if let (Ok(tab), Ok(draft), Ok(baseline)) = (
                                        selected_tab.lock(),
                                        draft_settings.lock(),
                                        viewport_baseline.lock(),
                                    ) {
                                        let dirty: std::collections::HashSet<SettingsTab> =
                                            SettingsTab::all()
                                                .iter()
                                                .filter(|&&t| {
                                                    section_is_dirty(t, &draft, &baseline)
                                                })
                                                .copied()
                                                .collect();
                                        (*tab, dirty)
                                    } else {
                                        (SettingsTab::General, Default::default())
                                    };

                                let filter: String = ctx
                                    .data(|d| d.get_temp(egui::Id::new("settings_search_query")))
                                    .unwrap_or_default();
                                let filter_lower = filter.to_lowercase();

                                ui.add_space(4.0);
                                for &tab in SettingsTab::all() {
                                    if !filter_lower.is_empty() {
                                        let matches =
                                            tab.label().to_lowercase().contains(&filter_lower)
                                                || tab
                                                    .subtitle()
                                                    .to_lowercase()
                                                    .contains(&filter_lower);
                                        if !matches {
                                            continue;
                                        }
                                    }
                                    let is_selected = tab == current_tab;
                                    let is_dirty = dirty_sections.contains(&tab);

                                    let (rect, resp) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), 36.0),
                                        egui::Sense::click(),
                                    );

                                    // Selection / hover background
                                    let bg = if is_selected {
                                        theme_colors.surface_raised
                                    } else if resp.hovered() {
                                        egui::Color32::from_rgba_unmultiplied(
                                            theme_colors.surface.r(),
                                            theme_colors.surface.g(),
                                            theme_colors.surface.b(),
                                            120,
                                        )
                                    } else {
                                        egui::Color32::TRANSPARENT
                                    };
                                    ui.painter().rect_filled(rect, 4.0, bg);

                                    // Selection accent bar
                                    if is_selected {
                                        ui.painter().rect_filled(
                                            egui::Rect::from_min_size(
                                                rect.min,
                                                egui::vec2(3.0, rect.height()),
                                            ),
                                            egui::CornerRadius::same(2),
                                            theme_colors.accent,
                                        );
                                    }

                                    // Icon
                                    let text_color = if is_selected {
                                        theme_colors.fg
                                    } else {
                                        theme_colors.fg_muted
                                    };
                                    ui.painter().text(
                                        rect.min + egui::vec2(14.0, rect.height() / 2.0),
                                        egui::Align2::LEFT_CENTER,
                                        tab.icon(),
                                        phosphor_font_id(15.0),
                                        text_color,
                                    );

                                    // Label
                                    ui.painter().text(
                                        rect.min + egui::vec2(36.0, rect.height() / 2.0),
                                        egui::Align2::LEFT_CENTER,
                                        tab.label(),
                                        egui::FontId::proportional(13.0),
                                        text_color,
                                    );

                                    // Dirty dot
                                    if is_dirty {
                                        ui.painter().circle_filled(
                                            rect.right_center() - egui::vec2(12.0, 0.0),
                                            3.0,
                                            theme_colors.accent,
                                        );
                                    }

                                    if resp.hovered() {
                                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if resp.clicked()
                                        && let Ok(mut t) = selected_tab.lock()
                                    {
                                        *t = tab;
                                    }
                                }
                            });
                    });

                // Central content area
                egui::CentralPanel::default()
                    .frame(egui::Frame::default().fill(theme_colors.bg))
                    .show_inside(ui, |ui| {
                        if let (Ok(current_tab), Ok(mut settings), Ok(mut events), Ok(baseline)) = (
                            selected_tab.lock(),
                            draft_settings.lock(),
                            viewport_events.lock(),
                            viewport_baseline.lock(),
                        ) {
                            Self::render_tab_content(
                                ui,
                                *current_tab,
                                &mut settings,
                                &baseline,
                                &theme_colors,
                                update_state_clone.as_ref(),
                                last_check_clone,
                                &current_version,
                                &mut events,
                                &open_plugin_settings_id,
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

        if let Ok(mut closed) = self.viewport_closed.lock()
            && *closed
        {
            self.open = false;
            *closed = false; // Reset for next time

            // Check if Apply was clicked (result will be Some)
            if let Ok(mut viewport_result) = self.viewport_result.lock() {
                result = viewport_result.take();
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
