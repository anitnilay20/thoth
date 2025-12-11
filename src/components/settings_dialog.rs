use crate::settings::Settings;
use eframe::egui;

/// Settings dialog with tabbed interface for comprehensive configuration
pub struct SettingsDialog {
    /// Whether the dialog is open
    pub open: bool,

    /// Currently selected tab
    selected_tab: SettingsTab,

    /// Draft settings being edited (not yet saved)
    draft_settings: Settings,

    /// Whether settings have been modified
    modified: bool,
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

impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            open: false,
            selected_tab: SettingsTab::General,
            draft_settings: Settings::default(),
            modified: false,
        }
    }
}

impl SettingsDialog {
    /// Open the settings dialog with current settings
    pub fn open(&mut self, current_settings: &Settings) {
        self.open = true;
        self.draft_settings = current_settings.clone();
        self.modified = false;
    }

    /// Close the settings dialog
    pub fn close(&mut self) {
        self.open = false;
        self.modified = false;
    }

    /// Render the settings dialog and return updated settings if saved
    pub fn show(&mut self, ctx: &egui::Context) -> Option<Settings> {
        let mut save_settings = None;
        let mut open = self.open;

        if !open {
            return None;
        }

        egui::Window::new("⚙ Settings")
            .open(&mut open)
            .default_width(800.0)
            .default_height(600.0)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                self.render_content(ui, &mut save_settings);
            });

        self.open = open;

        if !self.open {
            self.modified = false;
        }

        save_settings
    }

    /// Render settings directly without window wrapper (for standalone settings window)
    /// Always renders content and returns settings if save was clicked or cancel was clicked
    pub fn show_direct(&mut self, ctx: &egui::Context) -> Option<Settings> {
        let mut result = None;

        egui::CentralPanel::default().show(ctx, |ui| {
            result = self.render_content_standalone(ui);
        });

        result
    }

    fn render_content(&mut self, ui: &mut egui::Ui, save_settings: &mut Option<Settings>) {
        self.render_content_impl(ui, save_settings, false);
    }

    fn render_content_standalone(&mut self, ui: &mut egui::Ui) -> Option<Settings> {
        let mut result = None;
        self.render_content_impl(ui, &mut result, true);
        result
    }

    fn render_content_impl(
        &mut self,
        ui: &mut egui::Ui,
        save_settings: &mut Option<Settings>,
        standalone: bool,
    ) {
        // Use vertical layout to ensure proper full-height stretching
        ui.vertical(|ui| {
            // Top section: tabs and content (use available space)
            ui.horizontal(|ui| {
                // Left sidebar with tabs - fixed width, full height
                ui.vertical(|ui| {
                    ui.set_width(180.0);
                    ui.set_min_height(ui.available_height() - 60.0); // Reserve space for buttons
                    ui.add_space(8.0);

                    ui.selectable_value(&mut self.selected_tab, SettingsTab::General, "🏠 General");
                    ui.selectable_value(
                        &mut self.selected_tab,
                        SettingsTab::Appearance,
                        "🎨 Appearance",
                    );
                    ui.selectable_value(
                        &mut self.selected_tab,
                        SettingsTab::Performance,
                        "⚡ Performance",
                    );
                    ui.selectable_value(&mut self.selected_tab, SettingsTab::Viewer, "📄 Viewer");
                    ui.selectable_value(
                        &mut self.selected_tab,
                        SettingsTab::Shortcuts,
                        "⌨ Shortcuts",
                    );
                    ui.selectable_value(&mut self.selected_tab, SettingsTab::Updates, "🔄 Updates");
                    ui.selectable_value(
                        &mut self.selected_tab,
                        SettingsTab::Advanced,
                        "🔧 Advanced",
                    );
                });

                ui.separator();

                // Right content area with scroll - takes remaining space
                egui::ScrollArea::vertical()
                    .id_salt("settings_content")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.set_min_height(ui.available_height());

                        // Add padding around content
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.vertical(|ui| {
                                ui.set_max_width(ui.available_width() - 24.0);

                                match self.selected_tab {
                                    SettingsTab::General => self.render_general_tab(ui),
                                    SettingsTab::Appearance => self.render_appearance_tab(ui),
                                    SettingsTab::Performance => self.render_performance_tab(ui),
                                    SettingsTab::Viewer => self.render_viewer_tab(ui),
                                    SettingsTab::Shortcuts => self.render_shortcuts_tab(ui),
                                    SettingsTab::Updates => self.render_updates_tab(ui),
                                    SettingsTab::Advanced => self.render_advanced_tab(ui),
                                }
                            });
                        });
                    });
            });

            ui.add_space(8.0);
            ui.separator();

            // Bottom buttons - fixed height
            ui.horizontal(|ui| {
                ui.set_height(40.0);

                // Show modified indicator
                if self.modified {
                    ui.colored_label(egui::Color32::from_rgb(255, 165, 0), "● Modified");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        if !standalone {
                            self.close();
                        } else {
                            // In standalone mode, signal to close the window
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }

                    ui.add_space(8.0);

                    if ui.button("Reset to Defaults").clicked() {
                        self.draft_settings = Settings::default();
                        self.modified = true;
                    }

                    ui.add_space(8.0);

                    let save_button = ui.button("💾 Save");
                    if save_button.clicked() {
                        *save_settings = Some(self.draft_settings.clone());
                        if !standalone {
                            self.close();
                        }
                        // In standalone mode, don't close here - let the parent handle it after saving
                    }

                    // Make save button visually distinct if modified
                    if self.modified {
                        save_button.highlight();
                    }
                });
            });
        });
    }

    fn render_general_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("General Settings");
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Window").strong());
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Default Width:");
                if ui
                    .add(
                        egui::DragValue::new(&mut self.draft_settings.window.default_width)
                            .speed(10.0)
                            .range(400.0..=7680.0)
                            .suffix(" px"),
                    )
                    .changed()
                {
                    self.modified = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Default Height:");
                if ui
                    .add(
                        egui::DragValue::new(&mut self.draft_settings.window.default_height)
                            .speed(10.0)
                            .range(300.0..=4320.0)
                            .suffix(" px"),
                    )
                    .changed()
                {
                    self.modified = true;
                }
            });
        });

        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Updates").strong());
            ui.add_space(8.0);

            if ui
                .checkbox(
                    &mut self.draft_settings.updates.auto_check,
                    "Automatically check for updates",
                )
                .changed()
            {
                self.modified = true;
            }

            ui.horizontal(|ui| {
                ui.label("Check interval:");
                if ui
                    .add(
                        egui::DragValue::new(&mut self.draft_settings.updates.check_interval_hours)
                            .speed(1.0)
                            .range(1..=168)
                            .suffix(" hours"),
                    )
                    .changed()
                {
                    self.modified = true;
                }
            });
        });
    }

    fn render_appearance_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Appearance Settings");
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Theme").strong());
            ui.add_space(8.0);

            if ui
                .checkbox(&mut self.draft_settings.dark_mode, "Dark mode")
                .changed()
            {
                self.modified = true;
            }

            ui.label(
                "For detailed theme color customization, edit the theme section in settings.toml",
            );
        });

        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Font").strong());
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Font Size:");
                if ui
                    .add(
                        egui::Slider::new(&mut self.draft_settings.font_size, 8.0..=72.0)
                            .suffix(" pt"),
                    )
                    .changed()
                {
                    self.modified = true;
                }
            });

            ui.label("Live preview: Aa Bb Cc 123")
                .on_hover_text("This is how text will look at the selected size");
        });

        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("UI Elements").strong());
            ui.add_space(8.0);

            if ui
                .checkbox(&mut self.draft_settings.ui.show_toolbar, "Show toolbar")
                .changed()
            {
                self.modified = true;
            }

            if ui
                .checkbox(
                    &mut self.draft_settings.ui.show_status_bar,
                    "Show status bar",
                )
                .changed()
            {
                self.modified = true;
            }

            if ui
                .checkbox(
                    &mut self.draft_settings.ui.enable_animations,
                    "Enable animations",
                )
                .changed()
            {
                self.modified = true;
            }

            ui.horizontal(|ui| {
                ui.label("Sidebar width:");
                if ui
                    .add(
                        egui::DragValue::new(&mut self.draft_settings.ui.sidebar_width)
                            .speed(5.0)
                            .range(200.0..=1000.0)
                            .suffix(" px"),
                    )
                    .changed()
                {
                    self.modified = true;
                }
            });

            if ui
                .checkbox(
                    &mut self.draft_settings.ui.remember_sidebar_state,
                    "Remember sidebar state",
                )
                .changed()
            {
                self.modified = true;
            }
        });
    }

    fn render_performance_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Performance Settings");
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Caching").strong());
            ui.add_space(8.0);

            ui.label("Cache size controls how many parsed JSON values are kept in memory.");
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("LRU Cache Size:");
                if ui
                    .add(
                        egui::DragValue::new(&mut self.draft_settings.performance.cache_size)
                            .speed(10.0)
                            .range(1..=10000)
                            .suffix(" items"),
                    )
                    .changed()
                {
                    self.modified = true;
                }
            });

            ui.label(
                egui::RichText::new(
                    "Recommended: 100-1000. Higher values use more memory but improve performance.",
                )
                .italics()
                .small(),
            );
        });

        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("History").strong());
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Recent files to remember:");
                if ui
                    .add(
                        egui::DragValue::new(&mut self.draft_settings.performance.max_recent_files)
                            .speed(1.0)
                            .range(1..=100)
                            .suffix(" files"),
                    )
                    .changed()
                {
                    self.modified = true;
                }
            });
        });
    }

    fn render_viewer_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Viewer Settings");
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Display").strong());
            ui.add_space(8.0);

            if ui
                .checkbox(
                    &mut self.draft_settings.viewer.syntax_highlighting,
                    "Enable syntax highlighting",
                )
                .changed()
            {
                self.modified = true;
            }

            ui.label("Colorizes JSON keys, strings, numbers, and booleans for better readability.");
        });
    }

    fn render_shortcuts_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Keyboard Shortcuts");
        ui.add_space(16.0);

        ui.label("Keyboard shortcuts can be customized by editing the shortcuts section in settings.toml");
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Common Shortcuts").strong());
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Open file:");
                ui.label(egui::RichText::new("Cmd+O / Ctrl+O").monospace());
            });

            ui.horizontal(|ui| {
                ui.label("Close file:");
                ui.label(egui::RichText::new("Cmd+W / Ctrl+W").monospace());
            });

            ui.horizontal(|ui| {
                ui.label("Focus search:");
                ui.label(egui::RichText::new("Cmd+F / Ctrl+F").monospace());
            });

            ui.horizontal(|ui| {
                ui.label("Toggle theme:");
                ui.label(egui::RichText::new("Cmd+Shift+T / Ctrl+Shift+T").monospace());
            });
        });

        ui.add_space(16.0);

        ui.label(
            egui::RichText::new(
                "For full list and customization, see ~/.config/thoth/settings.toml",
            )
            .italics(),
        );
    }

    fn render_updates_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Updates");
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Application Updates").strong());
            ui.add_space(8.0);

            ui.label("Current Version: v0.2.16");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.button("Check for Updates").clicked() {
                    // TODO: Implement update check
                }
                ui.label("Check if a new version of Thoth is available");
            });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            ui.label(egui::RichText::new("Update Settings").strong());
            ui.add_space(8.0);

            if ui
                .checkbox(
                    &mut self.draft_settings.updates.auto_check,
                    "Automatically check for updates on startup",
                )
                .changed()
            {
                self.modified = true;
            }
            ui.label("Thoth will check for new versions when the application starts");
        });

        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Release Information").strong());
            ui.add_space(8.0);

            ui.label("View release notes and download updates at:");
            ui.hyperlink_to(
                "github.com/yourusername/thoth/releases",
                "https://github.com/yourusername/thoth/releases",
            );
        });
    }

    fn render_advanced_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Advanced Settings");
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Developer Options").strong());
            ui.add_space(8.0);

            #[cfg(feature = "profiling")]
            {
                if ui
                    .checkbox(&mut self.draft_settings.dev.show_profiler, "Show profiler")
                    .changed()
                {
                    self.modified = true;
                }
                ui.label("Enable performance profiling UI (requires profiling feature)");
            }

            #[cfg(not(feature = "profiling"))]
            {
                ui.label("Profiler not available (compile with --features profiling)");
            }
        });

        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new("Configuration").strong());
            ui.add_space(8.0);

            ui.label(format!("Config version: {}", self.draft_settings.version));

            if let Ok(path) = Settings::settings_file_path() {
                ui.horizontal(|ui| {
                    ui.label("Config file:");
                    ui.label(path.display().to_string());
                });

                if ui.button("📂 Open config folder").clicked() {
                    if let Some(parent) = path.parent() {
                        let _ = open::that(parent);
                    }
                }
            }
        });
    }
}
