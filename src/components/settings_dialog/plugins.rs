use eframe::egui;

use crate::PLUGIN_MANAGER;
use crate::components::common::card::{Card, CardAction, CardActionVariant, CardEvent, CardIcon};
use crate::components::common::toggle_switch::{
    ToggleSwitch, ToggleSwitchEvent, ToggleSwitchProps,
};
use crate::components::common::traits::StatelessComponent;
use crate::components::icon_button::{IconButton, IconButtonProps};
use crate::error::ErrorHandler;
use crate::error::ThothError;
use crate::notification::Notification;
use crate::notification::NotificationManager;
use crate::plugin::Plugin;
use crate::plugin::manager::PluginManager;
use crate::plugin::render_node::render_ui_node;
use crate::settings::{PluginNetworkPolicy, PluginSettings};
use crate::theme::{Theme, ThemeColors};

pub struct PluginsTab;

pub struct PluginsTabProps {
    pub plugin_settings: PluginSettings,
    pub active_plugin_settings: Option<String>,
}

pub enum PluginsTabEvent {
    EnablePlugins(bool),
    TogglePlugin {
        id: String,
        enabled: bool,
    },
    UninstallPlugin(String),
    // TODO: more specific events for different setting types so the UI doesn't have to re-render the entire settings page on every change
    // PluginUpdateSetting(String, Vec<PluginSettingData>),
    OpenSettingsForPlugin(Option<String>),
    UpdateNetworkPolicy {
        plugin_id: String,
        policy: PluginNetworkPolicy,
    },
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

        let Some(Some(pm)) = PLUGIN_MANAGER.get() else {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.add_space(24.0);
                    ui.horizontal(|ui| {
                        ui.add_space(24.0);
                        ui.label(
                            egui::RichText::new("Plugin manager not available.")
                                .color(colors.overlay1),
                        );
                    });
                });
            return output;
        };

        let all_plugins: Vec<&Plugin> = pm.get_all_plugin();

        let events = if let Some(active_id) = props.active_plugin_settings {
            if let Some(plugin) = pm.get_plugin_by_id(&active_id) {
                let plugin_network_setting =
                    match props.plugin_settings.network_policies.get(&active_id) {
                        Some(policy) => policy.clone(),
                        None => PluginNetworkPolicy::default(),
                    };

                Self::render_settings(ui, plugin, pm, colors, &plugin_network_setting)
            } else {
                vec![]
            }
        } else {
            Self::render_cards(ui, &props, &colors, all_plugins)
        };

        output.events = events;

        output
    }
}

impl PluginsTab {
    fn render_settings(
        ui: &mut egui::Ui,
        plugin: &Plugin,
        pm: &PluginManager,
        colors: ThemeColors,
        plugin_network_setting: &PluginNetworkPolicy,
    ) -> Vec<PluginsTabEvent> {
        let mut events = Vec::new();

        egui::ScrollArea::vertical()
            .auto_shrink([true, true])
            .show(ui, |ui| {
                egui::Frame::new().inner_margin(20).show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let back_button_response = IconButton::render(
                            ui,
                            IconButtonProps {
                                icon: egui_phosphor::regular::ARROW_LEFT,
                                frame: false,
                                tooltip: Some("Go Back"),
                                badge_color: None,
                                size: Some(egui::Vec2::new(20.0, 20.0)),
                                disabled: false,
                            },
                        );
                        ui.heading("Settings for plugin: ".to_string() + &plugin.name);

                        if back_button_response.clicked {
                            events.push(PluginsTabEvent::OpenSettingsForPlugin(None));
                        }
                    });
                    ui.separator();

                    if plugin.network.is_some() {
                        ui.add_space(8.0);
                        let network_events = Self::render_network_settings(
                            ui,
                            plugin,
                            colors,
                            &mut plugin_network_setting.clone(),
                        );
                        events.extend(network_events);
                        ui.add_space(8.0);
                    }

                    // Cache key: only compute WASM render once per plugin-settings session,
                    // not every egui frame (~60fps).
                    // Cache key: only compute WASM render once per plugin-settings session,
                    // not every egui frame (~60fps).
                    let cache_id = egui::Id::new(("plugin_settings_node", plugin.id.as_str()));
                    let cached: Option<crate::plugin::render_node::UiNode> =
                        ui.ctx().data(|d| d.get_temp(cache_id));

                    let node = cached.or_else(|| {
                        let result = pm
                            .open_plugin_settings(&plugin.id)
                            .and_then(|wps| wps.render_settings())
                            .and_then(|out| {
                                serde_json::from_str::<crate::plugin::render_node::UiNode>(
                                    &out.node_json,
                                )
                                .map_err(|e| ThothError::Unknown {
                                    message: format!("Failed to parse plugin settings JSON: {e}"),
                                })
                            });

                        match result {
                            Ok(node) => {
                                ui.ctx().data_mut(|d| d.insert_temp(cache_id, node.clone()));
                                Some(node)
                            }
                            Err(error) => {
                                eprintln!("Error rendering plugin settings: {error}");
                                NotificationManager::notify_error(Notification::new(
                                    "Error Loading Plugin Settings",
                                    &ErrorHandler::get_user_message(&error),
                                ));
                                None
                            }
                        }
                    });

                    if let Some(ref node) = node {
                        render_ui_node(ui, node, &mut vec![]);
                    }

                    // Clear cache when navigating away so stale data isn't shown
                    // if the same plugin is re-opened after its settings change.
                    if events
                        .iter()
                        .any(|e| matches!(e, PluginsTabEvent::OpenSettingsForPlugin(None)))
                    {
                        ui.ctx()
                            .data_mut(|d| d.remove::<crate::plugin::render_node::UiNode>(cache_id));
                    }

                    ui.add_space(8.0);
                });
            });

        events
    }

    fn render_cards(
        ui: &mut egui::Ui,
        props: &PluginsTabProps,
        colors: &ThemeColors,
        all_plugins: Vec<&Plugin>,
    ) -> Vec<PluginsTabEvent> {
        let mut events = Vec::new();

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
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_space(8.0);
                                    let toggle = ToggleSwitch::render(
                                        ui,
                                        ToggleSwitchProps {
                                            enabled: props.plugin_settings.enabled,
                                            hover_text: Some(
                                                "Enable or disable all plugins".into(),
                                            ),
                                        },
                                    );
                                    for event in toggle.events {
                                        let ToggleSwitchEvent::Toggled(v) = event;
                                        events.push(PluginsTabEvent::EnablePlugins(v));
                                    }
                                    ui.add_space(8.0);
                                    ui.label(
                                        egui::RichText::new("Enable plugins")
                                            .color(colors.overlay1),
                                    );
                                },
                            );
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

                            let meta =
                                format!("v{}  •  by {}", plugin.version, plugin.author);

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

                            let mut actions: Vec<CardAction> = vec![CardAction {
                                label: "Settings",
                                variant: CardActionVariant::Default,
                            }];
                            if !plugin.bundled {
                                actions.push(CardAction {
                                    label: "Uninstall",
                                    variant: CardActionVariant::Danger,
                                });
                            }

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
                                        events.push(PluginsTabEvent::TogglePlugin {
                                            id: plugin.id.clone(),
                                            enabled: v,
                                        });
                                    }
                                    CardEvent::ActionClicked(0) => {
                                        events.push(PluginsTabEvent::OpenSettingsForPlugin(
                                            Some(plugin.id.clone()),
                                        ));
                                    }
                                    CardEvent::ActionClicked(1) if !plugin.bundled => {
                                        events.push(PluginsTabEvent::UninstallPlugin(
                                            plugin.id.clone(),
                                        ));
                                    }
                                    _ => {}
                                }
                            }

                            ui.add_space(8.0);
                        }
                    });
                });
            });
        events
    }

    fn render_network_settings(
        ui: &mut egui::Ui,
        plugin: &Plugin,
        colors: ThemeColors,
        plugin_network_setting: &mut PluginNetworkPolicy,
    ) -> Vec<PluginsTabEvent> {
        let mut events = Vec::new();

        egui::Frame::new()
            .fill(colors.surface0)
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                let header = egui::CollapsingHeader::new(
                    egui::RichText::new("Network Settings").size(18.0),
                )
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(
                            "This plugin connects to external services. You can review and customize its network settings here.",
                        )
                        .color(colors.warning)
                        .size(12.0),
                    );
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(12.0);

                    Self::render_network_policy_form(
                        ui,
                        plugin,
                        &colors,
                        plugin_network_setting,
                        &mut events,
                    );
                });
                let _ = header;
            });

        events
    }

    /// Render an editable list of domain strings with per-row remove buttons
    /// and an add button at the bottom. Returns `true` if the list changed.
    fn render_domain_list(ui: &mut egui::Ui, domains: &mut Vec<String>) -> bool {
        let mut changed = false;
        let mut remove_idx: Option<usize> = None;

        for (i, domain) in domains.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                if ui.text_edit_singleline(domain).changed() {
                    changed = true;
                }
                if IconButton::render(
                    ui,
                    IconButtonProps {
                        icon: egui_phosphor::regular::MINUS,
                        frame: false,
                        tooltip: None,
                        badge_color: None,
                        size: None,
                        disabled: false,
                    },
                )
                .clicked
                {
                    remove_idx = Some(i);
                }
            });
        }

        if let Some(i) = remove_idx {
            domains.remove(i);
            changed = true;
        }

        if IconButton::render(
            ui,
            IconButtonProps {
                icon: egui_phosphor::regular::PLUS,
                frame: false,
                tooltip: None,
                badge_color: None,
                size: None,
                disabled: false,
            },
        )
        .clicked
        {
            domains.push(String::new());
            changed = true;
        }

        changed
    }

    fn render_network_policy_form(
        ui: &mut egui::Ui,
        plugin: &Plugin,
        colors: &ThemeColors,
        policy: &mut PluginNetworkPolicy,
        events: &mut Vec<PluginsTabEvent>,
    ) {
        ui.vertical(|ui| {
            ui.set_max_width(ui.available_width());

            // Show default network declarations from plugin.toml
            if let Some(network) = &plugin.network {
                ui.colored_label(
                    colors.text,
                    format!(
                        "📋 Plugin declares: {} domains | HTTPS required: {} | Rate limit: {} req/min",
                        network.allowed_domains.join(", "),
                        network.require_https,
                        network.rate_limit_rpm
                    ),
                );
                ui.add_space(12.0);
            }

            // ── Allowed Domains ────────────────────────────────────────
            ui.label(
                egui::RichText::new("Allowed Domains")
                    .strong()
                    .color(colors.text),
            );
            ui.label(
                egui::RichText::new(
                    "Domains this plugin can access (e.g., api.example.com, *.github.com)",
                )
                .color(colors.overlay1)
                .size(11.0),
            );

            if Self::render_domain_list(ui, &mut policy.allowed_domains) {
                events.push(PluginsTabEvent::UpdateNetworkPolicy {
                    plugin_id: plugin.id.clone(),
                    policy: policy.clone(),
                });
            }
            ui.add_space(8.0);

            // ── Blocked Domains ────────────────────────────────────────
            ui.label(
                egui::RichText::new("Blocked Domains")
                    .strong()
                    .color(colors.text),
            );
            ui.label(
                egui::RichText::new("Domains to block even if they appear in the allowed list")
                    .color(colors.overlay1)
                    .size(11.0),
            );

            if Self::render_domain_list(ui, &mut policy.blocked_domains) {
                events.push(PluginsTabEvent::UpdateNetworkPolicy {
                    plugin_id: plugin.id.clone(),
                    policy: policy.clone(),
                });
            }
            ui.add_space(8.0);

            // ── Require HTTPS ─────────────────────────────────────────
            ui.horizontal(|ui| {
                if ui.checkbox(&mut policy.require_https, "Require HTTPS").changed() {
                    events.push(PluginsTabEvent::UpdateNetworkPolicy {
                        plugin_id: plugin.id.clone(),
                        policy: policy.clone(),
                    });
                }
                ui.label(
                    egui::RichText::new("Block HTTP requests (only allow HTTPS)")
                        .color(colors.overlay1)
                        .size(11.0),
                );
            });
            ui.add_space(8.0);

            // ── Rate Limit (RPM) ──────────────────────────────────────
            ui.label(
                egui::RichText::new("Rate Limit (requests per minute)")
                    .strong()
                    .color(colors.text),
            );
            ui.label(
                egui::RichText::new(
                    "Maximum number of HTTP requests per minute (0 = unlimited)",
                )
                .color(colors.overlay1)
                .size(11.0),
            );

            let mut rate_limit_text = policy.rate_limit_rpm.to_string();
            if ui.text_edit_singleline(&mut rate_limit_text).changed() {
                if let Ok(value) = rate_limit_text.parse::<u32>() {
                    policy.rate_limit_rpm = value;
                    events.push(PluginsTabEvent::UpdateNetworkPolicy {
                        plugin_id: plugin.id.clone(),
                        policy: policy.clone(),
                    });
                }
            }
            ui.add_space(8.0);

            ui.group(|ui| {
                ui.colored_label(
                    colors.overlay1,
                    "These settings override the plugin's default network policy declared in plugin.toml",
                );
            });
        });
    }
}

// fn kv(key: &str, value: UiNode) -> UiNode {
//     UiNode::KeyValue {
//         key: key.to_string(),
//         value: Box::new(value),
//     }
// }

// impl Capability {
//     fn color(&self) -> &'static str {
//         match self {
//             Capability::FileLoader => "#10b981",
//             Capability::FileViewer => "#3b82f6",
//             Capability::DataSource => "#8b5cf6",
//             Capability::Exporter => "#f59e0b",
//             Capability::SearchProvider => "#ec4899",
//             Capability::NewUIComponent => "#06b6d4",
//         }
//     }
// }
