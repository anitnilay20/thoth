use crate::components::traits::ContextComponent;
use crate::helpers::{format_date, format_date_static};
use crate::update::{ReleaseInfo, UpdateState, UpdateStatus};
use eframe::egui;

/// Props passed down to the SettingsPanel (immutable, one-way binding)
pub struct SettingsPanelProps<'a> {
    pub show: bool,
    pub update_status: &'a UpdateStatus,
    pub current_version: &'a str,
}

/// Events emitted by the settings panel (bottom-to-top communication)
#[derive(Debug, Clone)]
pub enum SettingsPanelEvent {
    Close,
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

impl ContextComponent for SettingsPanel {
    type Props<'a> = SettingsPanelProps<'a>;
    type Output = SettingsPanelOutput;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        if !props.show {
            return SettingsPanelOutput { events: Vec::new() };
        }

        let mut events = Vec::new();
        self.render_ui(ctx, props, &mut events);

        SettingsPanelOutput { events }
    }
}

impl SettingsPanel {
    fn render_ui(
        &mut self,
        ctx: &egui::Context,
        props: SettingsPanelProps<'_>,
        events: &mut Vec<SettingsPanelEvent>,
    ) {
        let mut show = props.show;

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
        egui::Window::new(format!("{} Settings", egui_phosphor::regular::GEAR))
            .default_width(700.0)
            .default_height(600.0)
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .open(&mut show)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    Self::render_update_section(
                        ui,
                        props.update_status,
                        props.current_version,
                        events,
                    );
                });
            });

        // Emit close event if show state changed
        if show != props.show {
            events.push(SettingsPanelEvent::Close);
        }
    }

    fn render_update_section(
        ui: &mut egui::Ui,
        update_status: &UpdateStatus,
        current_version: &str,
        events: &mut Vec<SettingsPanelEvent>,
    ) {
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
                    .button(
                        egui::RichText::new(format!(
                            "{} Check for Updates",
                            egui_phosphor::regular::MAGNIFYING_GLASS
                        ))
                        .size(14.0),
                    )
                    .clicked()
                {
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

                if ui
                    .button(egui::RichText::new("⬇ Download Update").size(14.0))
                    .clicked()
                {
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
                ui.add(progress_bar);
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

                if ui
                    .button(egui::RichText::new("🔄 Try Again").size(14.0))
                    .clicked()
                {
                    events.push(SettingsPanelEvent::RetryUpdate);
                }
            }
        }
    }

    fn render_release_info(ui: &mut egui::Ui, release: &ReleaseInfo) {
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
                    .id_salt(egui::Id::new(("changelog", &release.tag_name)))
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
