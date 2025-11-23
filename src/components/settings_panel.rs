use crate::components::traits::StatefulComponent;
use crate::helpers::{format_date, format_date_static};
use crate::update::{ReleaseInfo, UpdateState, UpdateStatus};
use eframe::egui;

// UI constants
const BUTTON_FONT_SIZE: f32 = 14.0;

/// Props passed down to the SettingsPanel (immutable, one-way binding)
pub struct SettingsPanelProps<'a> {
    pub update_status: &'a UpdateStatus,
    pub current_version: &'a str,
}

/// Events emitted by the settings panel (bottom-to-top communication)
#[derive(Debug, Clone)]
pub enum SettingsPanelEvent {
    CheckForUpdates,
    DownloadUpdate,
    InstallUpdate,
    RetryUpdate,
}

pub struct SettingsPanelOutput {
    pub events: Vec<SettingsPanelEvent>,
}

#[derive(Default)]
pub struct SettingsPanel;

impl StatefulComponent for SettingsPanel {
    type Props<'a> = SettingsPanelProps<'a>;
    type Output = SettingsPanelOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let mut events = Vec::new();

        // Get theme colors from context
        let header_color = ui.ctx().memory(|mem| {
            if let Some(colors) = mem
                .data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
            {
                colors.sidebar_header
            } else {
                // Fallback color
                egui::Color32::from_rgb(153, 153, 153)
            }
        });

        // Header
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("SETTINGS")
                .size(11.0)
                .color(header_color)
                .strong(),
        );

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(8.0);

        Self::render_update_section(ui, props.update_status, props.current_version, &mut events);

        SettingsPanelOutput { events }
    }
}

impl SettingsPanel {
    fn render_update_section(
        ui: &mut egui::Ui,
        update_status: &UpdateStatus,
        current_version: &str,
        events: &mut Vec<SettingsPanelEvent>,
    ) {
        // Ensure the entire section takes full width
        ui.set_width(ui.available_width());

        ui.heading(egui::RichText::new("🔄 Updates").size(20.0));
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Current Version:").size(BUTTON_FONT_SIZE));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(current_version).size(16.0).strong());
                });
            });
        });
        ui.add_space(16.0);

        match &update_status.state {
            UpdateState::Idle => {
                match update_status.last_check {
                    Some(v) => {
                        ui.label(format!("Last check performed: {}", format_date_static(&v)))
                    }
                    None => ui.label("💤 No update check performed yet."),
                };
                ui.add_space(8.0);
                let check_button = ui.add_sized(
                    egui::vec2(ui.available_width(), 0.0),
                    egui::Button::new(
                        egui::RichText::new(format!(
                            "{} Check for Updates",
                            egui_phosphor::regular::MAGNIFYING_GLASS
                        ))
                        .size(BUTTON_FONT_SIZE),
                    ),
                );

                // Add accessibility info
                check_button.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        "Check for Updates",
                    )
                });

                if check_button.clicked() {
                    events.push(SettingsPanelEvent::CheckForUpdates);
                }
            }
            UpdateState::Checking => {
                ui.spinner();
                ui.label("Checking for updates...");
            }
            UpdateState::UpdateAvailable {
                latest_version,
                current_version: _,
                releases,
            } => {
                ui.colored_label(
                    egui::Color32::from_rgb(0, 200, 0),
                    format!("✨ Update available: {}", latest_version),
                );
                ui.add_space(16.0);

                let download_button = ui.add_sized(
                    egui::vec2(ui.available_width(), 0.0),
                    egui::Button::new(
                        egui::RichText::new("⬇ Download Update").size(BUTTON_FONT_SIZE),
                    ),
                );

                download_button.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        "Download Update",
                    )
                });

                if download_button.clicked() {
                    events.push(SettingsPanelEvent::DownloadUpdate);
                }
                ui.add_space(16.0);

                ui.separator();
                ui.add_space(24.0);
                ui.label(
                    egui::RichText::new("📦 Available Versions")
                        .size(16.0)
                        .strong(),
                );
                ui.add_space(16.0);

                for release in releases {
                    Self::render_release_info(ui, release);
                    ui.add_space(15.0);
                }
            }
            UpdateState::Downloading { progress, version } => {
                ui.label(format!("⬇ Downloading version {}...", version));
                ui.add_space(8.0);

                let progress_bar = egui::ProgressBar::new(progress / 100.0)
                    .show_percentage()
                    .animate(true);
                ui.add_sized(egui::vec2(ui.available_width(), 0.0), progress_bar);
            }
            UpdateState::ReadyToInstall { version, path: _ } => {
                ui.colored_label(
                    egui::Color32::from_rgb(0, 200, 0),
                    format!("✅ Version {} downloaded successfully!", version),
                );
                ui.add_space(16.0);

                ui.label("⚠ The application will restart after installation.");
                ui.add_space(8.0);

                let install_button = ui.add_sized(
                    egui::vec2(ui.available_width(), 0.0),
                    egui::Button::new(egui::RichText::new("🚀 Install Now").size(BUTTON_FONT_SIZE)),
                );

                install_button.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        "Install Now",
                    )
                });

                if install_button.clicked() {
                    events.push(SettingsPanelEvent::InstallUpdate);
                }
            }
            UpdateState::Installing => {
                ui.spinner();
                ui.label(format!(
                    "{} Installing update...",
                    egui_phosphor::regular::GEAR
                ));
                ui.add_space(8.0);
                ui.label("Please wait, the application will restart automatically.");
            }
            UpdateState::Error(error) => {
                ui.colored_label(egui::Color32::from_rgb(200, 0, 0), "❌ Update Error");
                ui.add_space(8.0);
                ui.label(error);
                ui.add_space(16.0);

                let retry_button = ui.add_sized(
                    egui::vec2(ui.available_width(), 0.0),
                    egui::Button::new(egui::RichText::new("🔄 Try Again").size(BUTTON_FONT_SIZE)),
                );

                retry_button.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        "Try Again",
                    )
                });

                if retry_button.clicked() {
                    events.push(SettingsPanelEvent::RetryUpdate);
                }
            }
        }
    }

    fn render_release_info(ui: &mut egui::Ui, release: &ReleaseInfo) {
        ui.group(|ui| {
            ui.set_width(ui.available_width());

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&release.name).size(16.0).strong());
                if release.prerelease {
                    ui.label(
                        egui::RichText::new("⚠ PRE-RELEASE")
                            .color(egui::Color32::from_rgb(255, 165, 0))
                            .small(),
                    );
                }
            });

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Version:").strong());
                ui.label(&release.tag_name);
                ui.separator();
                ui.add_space(8.0);
                ui.label(egui::RichText::new("📅 Published:").strong());
                ui.label(format_date(&release.published_at));
            });

            ui.add_space(8.0);

            if !release.body.is_empty() {
                ui.label(egui::RichText::new("Changelog:").strong());
                ui.add_space(3.0);

                egui::ScrollArea::vertical()
                    .id_salt(egui::Id::new(("changelog", &release.tag_name)))
                    .max_height(200.0)
                    .show(ui, |ui| {
                        ui.label(&release.body);
                    });
            }

            ui.add_space(8.0);

            if ui
                .add_sized(
                    egui::vec2(ui.available_width(), 0.0),
                    egui::Button::new(egui::RichText::new("🔗 View on GitHub").size(12.0)),
                )
                .clicked()
            {
                let _ = open::that(&release.html_url);
            }
        });
    }
}
