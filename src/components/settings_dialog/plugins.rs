use eframe::egui;

use crate::PLUGIN_MANAGER;
use crate::components::card::{Card, CardIcon, CardProps};
use crate::components::toggle_switch::{self, ToggleSwitchProps};
use crate::settings::PluginSettings;
use crate::{
    components::traits::StatelessComponent,
    plugin::{Capability, Plugin},
};

pub struct PluginsTab;

pub struct PluginsTabProps {
    pub plugin_setting: PluginSettings,
}

pub enum PluginsTabEvent {
    EnablePlugins(bool),
}

pub struct PluginsTabOutput {
    pub events: Vec<PluginsTabEvent>,
}

impl StatelessComponent for PluginsTab {
    type Props<'a> = PluginsTabProps;

    type Output = PluginsTabOutput;

    fn render(ui: &mut eframe::egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut pluggin_tab_output = PluginsTabOutput { events: Vec::new() };

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        ui.set_max_width(ui.available_width() - 24.0);
                        ui.horizontal(|ui| {
                            ui.heading("Plugins");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(24.0);
                                let toggle_switch = toggle_switch::ToggleSwitch::render(
                                    ui,
                                    ToggleSwitchProps {
                                        enabled: props.plugin_setting.enabled,
                                        hover_text: Some("Enable or disable all plugins".into()),
                                    },
                                );
                                for event in toggle_switch.events {
                                    match event {
                                        toggle_switch::ToggleSwitchEvent::Toggled(toggled) => {
                                            pluggin_tab_output
                                                .events
                                                .push(PluginsTabEvent::EnablePlugins(toggled));
                                        }
                                    }
                                }
                            });
                        });
                        ui.add_space(16.0);
                        Self::render_all_capability(ui)
                    });
                });
            });

        pluggin_tab_output
    }
}

impl PluginsTab {
    fn render_all_capability(ui: &mut eframe::egui::Ui) {
        [
            Capability::FileLoader,
            Capability::FileViewer,
            Capability::DataSource,
            Capability::Exporter,
        ]
        .iter()
        .for_each(|c| {
            if let Some(Some(plugin_manager)) = PLUGIN_MANAGER.get() {
                Self::render_section(
                    ui,
                    &c.to_string(),
                    plugin_manager.get_all_plugin_by_capability(c.clone()),
                );
            }
        });
    }

    fn render_section(ui: &mut eframe::egui::Ui, heading: &str, plugins: Vec<&Plugin>) {
        if plugins.is_empty() {
            return;
        }

        ui.label(egui::RichText::new(heading).size(16.0));
        ui.add_space(8.0);
        plugins.iter().for_each(|plugin| {
            let icon = match &plugin.icon_path {
                Some(p) => CardIcon::Path(p.as_path()),
                None => CardIcon::Color(ui.visuals().weak_text_color()),
            };
            let _output = Card::render(
                ui,
                CardProps {
                    title: &plugin.name,
                    subtitle: &plugin.description,
                    meta: Some(&format!("v{} | by {}", plugin.version, plugin.author)),
                    is_enabled: Some(false),
                    icon,
                    actions: &[],
                },
            );
        });
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);
    }
}
