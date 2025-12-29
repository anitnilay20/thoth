use crate::components::traits::StatelessComponent;
use crate::settings::UpdateSettings;
use crate::theme::ThemeColors;
use crate::update::UpdateState;
use eframe::egui;

/// Updates settings tab component
pub struct UpdatesTab;

/// Props for the Updates tab
pub struct UpdatesTabProps<'a> {
    pub update_settings: &'a UpdateSettings,
    pub update_state: Option<&'a UpdateState>,
    pub current_version: &'a str,
    pub theme_colors: &'a ThemeColors,
}

/// Events emitted by the Updates tab
#[derive(Debug, Clone)]
pub enum UpdatesTabEvent {
    AutoCheckChanged(bool),
    CheckIntervalChanged(u64),
    CheckForUpdates,
    DownloadUpdate,
    InstallUpdate,
}

/// Output from the Updates tab
pub struct UpdatesTabOutput {
    pub events: Vec<UpdatesTabEvent>,
}

impl StatelessComponent for UpdatesTab {
    type Props<'a> = UpdatesTabProps<'a>;
    type Output = UpdatesTabOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                // Add padding to the content
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        ui.set_max_width(ui.available_width() - 24.0);

                        ui.heading("Updates");
                        ui.add_space(16.0);

                        // Current version section
                        Self::render_current_version(ui, props.current_version, props.theme_colors);

                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Update settings section
                        Self::render_update_settings(
                            ui,
                            props.update_settings,
                            props.theme_colors,
                            &mut events,
                        );

                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Update status section
                        Self::render_update_status(
                            ui,
                            props.update_state,
                            &mut events,
                            props.theme_colors,
                        );

                        ui.add_space(16.0);
                    });
                });
            });

        UpdatesTabOutput { events }
    }
}

impl UpdatesTab {
    fn render_current_version(ui: &mut egui::Ui, version: &str, theme_colors: &ThemeColors) {
        ui.label(egui::RichText::new("Version Information").size(16.0));
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Current version:");
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(version)
                    .strong()
                    .color(theme_colors.text),
            );
        });
    }

    fn render_update_settings(
        ui: &mut egui::Ui,
        update_settings: &UpdateSettings,
        theme_colors: &ThemeColors,
        events: &mut Vec<UpdatesTabEvent>,
    ) {
        ui.label(egui::RichText::new("Update Settings").size(16.0));
        ui.add_space(8.0);

        // Auto-check for updates
        ui.horizontal(|ui| {
            let mut auto_check = update_settings.auto_check;
            let checkbox = ui.checkbox(&mut auto_check, "");

            if checkbox.changed() {
                events.push(UpdatesTabEvent::AutoCheckChanged(auto_check));
            }

            if checkbox.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.label("Automatically check for updates");
        });

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Check for updates periodically in the background")
                .size(12.0)
                .color(theme_colors.overlay1),
        );

        ui.add_space(12.0);

        // Check interval
        ui.horizontal(|ui| {
            ui.label("Check interval:");
            ui.add_space(8.0);

            let mut check_interval = update_settings.check_interval_hours;
            let slider = egui::Slider::new(&mut check_interval, 1..=168)
                .suffix(" hours")
                .min_decimals(0)
                .max_decimals(0);

            let response = ui.add_sized([ui.available_width().min(300.0), 20.0], slider);

            if response.changed() {
                events.push(UpdatesTabEvent::CheckIntervalChanged(check_interval));
            }

            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        });

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("How often to check for new versions (1 hour to 7 days)")
                .size(12.0)
                .color(theme_colors.overlay1),
        );
    }

    fn render_update_status(
        ui: &mut egui::Ui,
        update_state: Option<&UpdateState>,
        events: &mut Vec<UpdatesTabEvent>,
        theme_colors: &ThemeColors,
    ) {
        ui.label(egui::RichText::new("Update Status").size(16.0));
        ui.add_space(8.0);

        match update_state {
            None | Some(UpdateState::Idle) => {
                ui.label("No update check performed yet");
                ui.add_space(8.0);

                if ui.button("Check for Updates").clicked() {
                    events.push(UpdatesTabEvent::CheckForUpdates);
                }
            }
            Some(UpdateState::Checking) => {
                ui.label("Checking for updates...");
                ui.add_space(8.0);
                ui.spinner();
            }
            Some(UpdateState::UpdateAvailable {
                latest_version,
                current_version,
                releases,
            }) => {
                ui.label(
                    egui::RichText::new(format!(
                        "Update available: {} -> {}",
                        current_version, latest_version
                    ))
                    .strong()
                    .color(theme_colors.text),
                );
                ui.add_space(8.0);

                if let Some(release) = releases.first() {
                    ui.label(format!("Release: {}", release.name));
                    ui.add_space(4.0);

                    // Show release notes in a scrollable area
                    if !release.body.is_empty() {
                        ui.label("Release Notes:");
                        ui.add_space(4.0);

                        egui::Frame::default()
                            .fill(ui.visuals().faint_bg_color)
                            .inner_margin(8.0)
                            .corner_radius(4.0)
                            .show(ui, |ui| {
                                ui.label(&release.body);
                            });

                        ui.add_space(8.0);
                    }

                    if ui.button("Download Update").clicked() {
                        events.push(UpdatesTabEvent::DownloadUpdate);
                    }
                }
            }
            Some(UpdateState::Downloading { progress, version }) => {
                ui.label(format!("Downloading version {}...", version));
                ui.add_space(8.0);

                let progress_bar = egui::ProgressBar::new(*progress).show_percentage();
                ui.add(progress_bar);
            }
            Some(UpdateState::ReadyToInstall { version, .. }) => {
                ui.label(
                    egui::RichText::new(format!("Update {} ready to install", version))
                        .strong()
                        .color(theme_colors.text),
                );
                ui.add_space(8.0);

                if ui.button("Install and Restart").clicked() {
                    events.push(UpdatesTabEvent::InstallUpdate);
                }
            }
            Some(UpdateState::Installing) => {
                ui.label("Installing update...");
                ui.add_space(8.0);
                ui.spinner();
            }
            Some(UpdateState::Error(error)) => {
                ui.label(
                    egui::RichText::new(format!("Error: {}", error))
                        .color(ui.visuals().error_fg_color),
                );
                ui.add_space(8.0);

                if ui.button("Retry").clicked() {
                    events.push(UpdatesTabEvent::CheckForUpdates);
                }
            }
        }
    }
}
