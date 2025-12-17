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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
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
            selected_tab: SettingsTab::Appearance,
            draft_settings: Settings::default(),
            viewport_result: Arc::new(Mutex::new(None)),
            viewport_draft: Arc::new(Mutex::new(Settings::default())),
            viewport_closed: Arc::new(Mutex::new(false)),
            viewport_selected_tab: Arc::new(Mutex::new(SettingsTab::Appearance)),
        }
    }
}

impl SettingsDialog {
    /// Open the settings dialog with current settings
    pub fn open(&mut self, current_settings: &Settings) {
        self.open = true;
        self.draft_settings = current_settings.clone();

        // Update viewport_draft with current settings
        if let Ok(mut draft) = self.viewport_draft.lock() {
            *draft = current_settings.clone();
        }

        // Reset closed flag
        if let Ok(mut closed) = self.viewport_closed.lock() {
            *closed = false;
        }

        // Update selected tab
        if let Ok(mut tab) = self.viewport_selected_tab.lock() {
            *tab = self.selected_tab;
        }
    }

    /// Close the settings dialog
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Get the current draft settings (for live preview)
    pub fn get_draft_settings(&self) -> Option<Settings> {
        if self.open {
            if let Ok(draft) = self.viewport_draft.lock() {
                Some(draft.clone())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Render settings in a separate viewport window (returns new settings if Apply clicked)
    pub fn show_viewport(&mut self, ctx: &egui::Context) -> Option<Settings> {
        if !self.open {
            return None;
        }

        let viewport_id = egui::ViewportId::from_hash_of("thoth_settings");

        // Clone Arc for use in the closure
        let viewport_result = Arc::clone(&self.viewport_result);
        let viewport_closed = Arc::clone(&self.viewport_closed);
        let draft_settings = Arc::clone(&self.viewport_draft);
        let selected_tab = Arc::clone(&self.viewport_selected_tab);

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
                    .frame(egui::Frame::default().fill(theme_colors.base).inner_margin(
                        egui::Margin {
                            left: 24,
                            right: 24,
                            top: 24,
                            bottom: 0,
                        },
                    ))
                    .show(ctx, |ui| {
                        // Constrain max width to prevent overflow
                        ui.set_max_width(ui.available_width());

                        if let (Ok(current_tab), Ok(mut settings)) =
                            (selected_tab.lock(), draft_settings.lock())
                        {
                            match *current_tab {
                                SettingsTab::General => {
                                    GeneralTab::render(ui, &mut settings, &theme_colors)
                                }
                                SettingsTab::Appearance => {
                                    AppearanceTab::render(ui, &mut settings, &theme_colors)
                                }
                                SettingsTab::Performance => {
                                    PerformanceTab::render(ui, &mut settings, &theme_colors)
                                }
                                SettingsTab::Viewer => {
                                    ViewerTab::render(ui, &mut settings, &theme_colors)
                                }
                                SettingsTab::Shortcuts => {
                                    ShortcutsTab::render(ui, &mut settings, &theme_colors)
                                }
                                SettingsTab::Updates => {
                                    UpdatesTab::render(ui, &mut settings, &theme_colors)
                                }
                                SettingsTab::Advanced => {
                                    AdvancedTab::render(ui, &mut settings, &theme_colors)
                                }
                            }
                        }
                    });

                // If Apply was clicked, store result and close viewport
                if let Some(settings) = new_settings {
                    if let Ok(mut result) = viewport_result.lock() {
                        *result = Some(settings);
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            },
        );

        // Check if viewport was closed/cancelled
        if let Ok(mut closed) = self.viewport_closed.lock() {
            if *closed {
                self.open = false;
                *closed = false;

                // Revert draft settings to original
                // (main window will stop using draft and go back to self.settings)
                return None;
            }
        }

        // Check if we have a result from the viewport
        if let Ok(mut result) = self.viewport_result.lock() {
            if let Some(settings) = result.take() {
                self.open = false;
                self.draft_settings = settings.clone();

                // Update viewport_draft to match
                if let Ok(mut draft) = self.viewport_draft.lock() {
                    *draft = settings.clone();
                }

                return Some(settings);
            }
        }

        None
    }

    /// Render settings directly without window wrapper (for standalone settings window)
    pub fn show_direct(&mut self, ctx: &egui::Context) -> Option<Settings> {
        // Apply theme from draft settings so changes preview in real-time
        theme::apply_theme(ctx, &self.draft_settings);

        // Get theme colors
        let theme_colors = ctx.memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| {
                    theme::Theme::for_dark_mode(ctx.style().visuals.dark_mode).colors()
                })
        });

        let mut result = None;

        // Top panel with title and buttons
        egui::TopBottomPanel::top("settings_top")
            .frame(
                egui::Frame::default()
                    .fill(theme_colors.crust)
                    .inner_margin(egui::Margin::symmetric(16, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Edit settings.toml button
                        let btn = ui.button(
                            egui::RichText::new("Edit settings in settings.toml").size(13.0),
                        );
                        if btn.hovered() {
                            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if btn.clicked() {
                            if let Ok(path) = Settings::settings_file_path() {
                                let _ = open::that(path);
                            }
                        }
                    });
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
                        result = Some(self.draft_settings.clone());
                    }

                    ui.add_space(8.0);

                    let cancel_btn = ui.button(egui::RichText::new("Cancel").size(14.0));
                    if cancel_btn.hovered() {
                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if cancel_btn.clicked() {
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
                    let is_selected = self.selected_tab == *tab;

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
                            self.selected_tab = *tab;
                        }
                    });

                    ui.add_space(8.0);
                }
            });

        // Central content area
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(theme_colors.base)
                    .inner_margin(egui::Margin {
                        left: 24,
                        right: 24,
                        top: 24,
                        bottom: 0,
                    }),
            )
            .show(ctx, |ui| {
                // Constrain max width to prevent overflow
                ui.set_max_width(ui.available_width());

                match self.selected_tab {
                    SettingsTab::General => {
                        GeneralTab::render(ui, &mut self.draft_settings, &theme_colors)
                    }
                    SettingsTab::Appearance => {
                        AppearanceTab::render(ui, &mut self.draft_settings, &theme_colors)
                    }
                    SettingsTab::Performance => {
                        PerformanceTab::render(ui, &mut self.draft_settings, &theme_colors)
                    }
                    SettingsTab::Viewer => {
                        ViewerTab::render(ui, &mut self.draft_settings, &theme_colors)
                    }
                    SettingsTab::Shortcuts => {
                        ShortcutsTab::render(ui, &mut self.draft_settings, &theme_colors)
                    }
                    SettingsTab::Updates => {
                        UpdatesTab::render(ui, &mut self.draft_settings, &theme_colors)
                    }
                    SettingsTab::Advanced => {
                        AdvancedTab::render(ui, &mut self.draft_settings, &theme_colors)
                    }
                }
            });

        result
    }
}

/// Props for SettingsDialog when used as a ContextComponent
pub struct SettingsDialogProps {
    // No props needed - SettingsDialog manages its own state
}

/// Output from SettingsDialog
pub struct SettingsDialogOutput {
    /// New settings if Apply was clicked
    pub new_settings: Option<Settings>,
}

impl ContextComponent for SettingsDialog {
    type Props<'a> = SettingsDialogProps;
    type Output = SettingsDialogOutput;

    fn render(&mut self, ctx: &egui::Context, _props: Self::Props<'_>) -> Self::Output {
        // Apply theme from draft settings so changes preview in real-time
        theme::apply_theme(ctx, &self.draft_settings);

        let mut result = None;

        egui::Window::new("Settings")
            .default_size([900.0, 600.0])
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                // Get theme colors
                let theme_colors = ctx.memory(|mem| {
                    mem.data
                        .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                        .unwrap_or_else(|| {
                            theme::Theme::for_dark_mode(ctx.style().visuals.dark_mode).colors()
                        })
                });

                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Edit settings.toml button
                        let btn = ui.button(
                            egui::RichText::new("Edit settings in settings.toml").size(13.0),
                        );
                        if btn.hovered() {
                            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if btn.clicked() {
                            if let Ok(path) = Settings::settings_file_path() {
                                let _ = open::that(path);
                            }
                        }
                    });
                });

                ui.separator();

                ui.horizontal(|ui| {
                    // Sidebar with tabs
                    ui.vertical(|ui| {
                        ui.set_width(180.0);
                        ui.add_space(8.0);

                        for tab in SettingsTab::all() {
                            let is_selected = self.selected_tab == *tab;

                            let response = ui.selectable_label(is_selected, tab.label());
                            if response.hovered() {
                                ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if response.clicked() {
                                self.selected_tab = *tab;
                            }
                        }
                    });

                    ui.separator();

                    // Content area
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            ui.set_max_width(ui.available_width());

                            match self.selected_tab {
                                SettingsTab::General => {
                                    GeneralTab::render(ui, &mut self.draft_settings, &theme_colors)
                                }
                                SettingsTab::Appearance => AppearanceTab::render(
                                    ui,
                                    &mut self.draft_settings,
                                    &theme_colors,
                                ),
                                SettingsTab::Performance => PerformanceTab::render(
                                    ui,
                                    &mut self.draft_settings,
                                    &theme_colors,
                                ),
                                SettingsTab::Viewer => {
                                    ViewerTab::render(ui, &mut self.draft_settings, &theme_colors)
                                }
                                SettingsTab::Shortcuts => ShortcutsTab::render(
                                    ui,
                                    &mut self.draft_settings,
                                    &theme_colors,
                                ),
                                SettingsTab::Updates => {
                                    UpdatesTab::render(ui, &mut self.draft_settings, &theme_colors)
                                }
                                SettingsTab::Advanced => {
                                    AdvancedTab::render(ui, &mut self.draft_settings, &theme_colors)
                                }
                            }
                        });
                });

                ui.separator();

                // Bottom buttons
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let apply_btn = ui.button(egui::RichText::new("Apply").size(14.0));
                        if apply_btn.hovered() {
                            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if apply_btn.clicked() {
                            result = Some(self.draft_settings.clone());
                        }

                        ui.add_space(8.0);

                        let cancel_btn = ui.button(egui::RichText::new("Cancel").size(14.0));
                        if cancel_btn.hovered() {
                            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if cancel_btn.clicked() {
                            self.open = false;
                        }
                    });
                });
            });

        SettingsDialogOutput {
            new_settings: result,
        }
    }
}
