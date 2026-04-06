use eframe::egui;

use crate::PLUGIN_MANAGER;
use crate::components::common::card::{Card, CardAction, CardActionVariant, CardEvent, CardIcon};
use crate::components::common::toggle_switch::{
    ToggleSwitch, ToggleSwitchEvent, ToggleSwitchProps,
};
use crate::components::common::traits::StatelessComponent;
use crate::plugin::Plugin;
use crate::settings::PluginSettings;
use crate::theme::{Theme, ThemeColors};

pub struct PluginsTab;

pub struct PluginsTabProps {
    pub plugin_settings: PluginSettings,
}

pub enum PluginsTabEvent {
    EnablePlugins(bool),
    TogglePlugin { id: String, enabled: bool },
    UninstallPlugin(String),
}

pub struct PluginsTabOutput {
    pub events: Vec<PluginsTabEvent>,
}

impl StatelessComponent for PluginsTab {
    type Props<'a> = PluginsTabProps;
    type Output = PluginsTabOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut output = PluginsTabOutput { events: Vec::new() };

        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| Theme::default().colors())
        });

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        ui.set_max_width(ui.available_width() - 24.0);

                        // ── Header ────────────────────────────────────────────
                        ui.horizontal(|ui| {
                            ui.heading("Plugins");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.add_space(8.0);
                                let toggle = ToggleSwitch::render(
                                    ui,
                                    ToggleSwitchProps {
                                        enabled: props.plugin_settings.enabled,
                                        hover_text: Some("Enable or disable all plugins".into()),
                                    },
                                );
                                for event in toggle.events {
                                    let ToggleSwitchEvent::Toggled(v) = event;
                                    output.events.push(PluginsTabEvent::EnablePlugins(v));
                                }
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("Enable plugins").color(colors.overlay1),
                                );
                            });
                        });

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                "Plugins extend Thoth with new file formats, data sources, and export targets.",
                            )
                            .color(colors.overlay1)
                            .size(12.0),
                        );
                        ui.add_space(20.0);
                        ui.separator();
                        ui.add_space(20.0);

                        // ── Plugin list ───────────────────────────────────────
                        let Some(Some(pm)) = PLUGIN_MANAGER.get() else {
                            ui.label(
                                egui::RichText::new("Plugin manager not available.")
                                    .color(colors.overlay1),
                            );
                            return;
                        };

                        // Collect all unique plugins (a plugin may appear under
                        // multiple capabilities — deduplicate by id).
                        let mut seen_ids = std::collections::HashSet::new();
                        let mut all_plugins: Vec<&Plugin> = Vec::new();
                        for cap in [
                            crate::plugin::Capability::FileLoader,
                            crate::plugin::Capability::FileViewer,
                            crate::plugin::Capability::DataSource,
                            crate::plugin::Capability::Exporter,
                        ] {
                            for p in pm.get_all_plugin_by_capability(cap) {
                                if seen_ids.insert(p.id.clone()) {
                                    all_plugins.push(p);
                                }
                            }
                        }

                        if all_plugins.is_empty() {
                            ui.label(
                                egui::RichText::new("No plugins installed.")
                                    .color(colors.overlay1),
                            );
                            return;
                        }

                        for plugin in all_plugins {
                            let is_enabled = !props
                                .plugin_settings
                                .disabled_plugin_ids
                                .contains(&plugin.id);

                            let meta = format!("v{}  •  by {}", plugin.version, plugin.author);

                            let cap_tag_strings: Vec<String> = plugin
                                .capabilities
                                .iter()
                                .map(|c| c.to_string())
                                .collect();
                            let cap_tags: Vec<&str> =
                                cap_tag_strings.iter().map(String::as_str).collect();

                            let icon = match &plugin.icon_path {
                                Some(p) => CardIcon::Path(p.as_path()),
                                None => CardIcon::Color(colors.overlay1),
                            };

                            // Bundled plugins: only toggle. User-installed: toggle + uninstall.
                            let actions: Vec<CardAction<'_>> = if plugin.bundled {
                                vec![]
                            } else {
                                vec![CardAction {
                                    label: "Uninstall",
                                    variant: CardActionVariant::Danger,
                                }]
                            };

                            let card_output = Card::render(
                                ui,
                                crate::components::common::card::CardProps {
                                    title: &plugin.name,
                                    subtitle: &plugin.description,
                                    meta: Some(&meta),
                                    tags: &cap_tags,
                                    is_enabled: Some(is_enabled),
                                    icon,
                                    actions: &actions,
                                },
                            );

                            for event in card_output.events {
                                match event {
                                    CardEvent::Toggled(v) => {
                                        output.events.push(PluginsTabEvent::TogglePlugin {
                                            id: plugin.id.clone(),
                                            enabled: v,
                                        });
                                    }
                                    CardEvent::ActionClicked(0) if !plugin.bundled => {
                                        output.events.push(PluginsTabEvent::UninstallPlugin(plugin.id.clone()));
                                    }
                                    _ => {}
                                }
                            }

                            ui.add_space(8.0);
                        }
                    });
                });
            });

        output
    }
}
