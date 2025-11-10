use crate::helpers::{format_date, format_date_static};
use crate::update::{ReleaseInfo, UpdateState, UpdateStatus};
use eframe::egui;

#[derive(Debug, Clone)]
pub enum SettingsAction {
    CheckForUpdates,
    DownloadUpdate,
    InstallUpdate,
    RetryUpdate,
}

#[derive(Default)]
pub struct SettingsPanel {
    pub show: bool,
}

impl SettingsPanel {
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        update_status: &UpdateStatus,
        current_version: &str,
    ) -> Option<SettingsAction> {
        if !self.show {
            return None;
        }

        let mut action = None;
        let mut show = self.show;

        // Draw semi-transparent backdrop
        egui::Area::new("settings_backdrop".into())
            .fixed_pos(egui::pos2(0.0, 0.0))
            .interactable(true)
            .order(egui::Order::Background)
            .show(ctx, |ui| {
                let screen_rect = ctx.screen_rect();
                let painter = ui.painter();
                painter.rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(180));

                // Consume clicks on the backdrop to close settings
                let response = ui.allocate_response(screen_rect.size(), egui::Sense::click());
                if response.clicked() {
                    show = false;
                }
            });

        // Draw settings window on top
        egui::Window::new("⚙ Settings")
            .default_width(700.0)
            .default_height(600.0)
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut show)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    action = Self::render_update_section_static(ui, update_status, current_version);
                });
            });

        self.show = show;
        action
    }

    fn render_update_section_static(
        ui: &mut egui::Ui,
        update_status: &UpdateStatus,
        current_version: &str,
    ) -> Option<SettingsAction> {
        ui.heading(egui::RichText::new("🔄 Updates").size(20.0));
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Current Version:").size(14.0));
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
                if ui
                    .button(egui::RichText::new("🔍 Check for Updates").size(14.0))
                    .clicked()
                {
                    Some(SettingsAction::CheckForUpdates)
                } else {
                    None
                }
            }
            UpdateState::Checking => {
                ui.spinner();
                ui.label("Checking for updates...");
                None
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

                let action = if ui
                    .button(egui::RichText::new("⬇ Download Update").size(14.0))
                    .clicked()
                {
                    Some(SettingsAction::DownloadUpdate)
                } else {
                    None
                };
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
                    Self::render_release_info_static(ui, release);
                    ui.add_space(15.0);
                }

                action
            }
            UpdateState::Downloading { progress, version } => {
                ui.label(format!("⬇ Downloading version {}...", version));
                ui.add_space(8.0);

                let progress_bar = egui::ProgressBar::new(progress / 100.0)
                    .show_percentage()
                    .animate(true);
                ui.add(progress_bar);
                None
            }
            UpdateState::ReadyToInstall { version, path: _ } => {
                ui.colored_label(
                    egui::Color32::from_rgb(0, 200, 0),
                    format!("✅ Version {} downloaded successfully!", version),
                );
                ui.add_space(16.0);

                ui.label("⚠ The application will restart after installation.");
                ui.add_space(8.0);

                if ui
                    .button(egui::RichText::new("🚀 Install Now").size(14.0))
                    .clicked()
                {
                    Some(SettingsAction::InstallUpdate)
                } else {
                    None
                }
            }
            UpdateState::Installing => {
                ui.spinner();
                ui.label("⚙ Installing update...");
                ui.add_space(8.0);
                ui.label("Please wait, the application will restart automatically.");
                None
            }
            UpdateState::Error(error) => {
                ui.colored_label(egui::Color32::from_rgb(200, 0, 0), "❌ Update Error");
                ui.add_space(8.0);
                ui.label(error);
                ui.add_space(16.0);

                if ui
                    .button(egui::RichText::new("🔄 Try Again").size(14.0))
                    .clicked()
                {
                    Some(SettingsAction::RetryUpdate)
                } else {
                    None
                }
            }
        }
    }

    fn render_release_info_static(ui: &mut egui::Ui, release: &ReleaseInfo) {
        ui.group(|ui| {
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
                    .max_height(200.0)
                    .show(ui, |ui| {
                        ui.label(&release.body);
                    });
            }

            ui.add_space(8.0);

            if ui
                .button(egui::RichText::new("🔗 View on GitHub").size(12.0))
                .clicked()
            {
                let _ = open::that(&release.html_url);
            }
        });
    }
}
