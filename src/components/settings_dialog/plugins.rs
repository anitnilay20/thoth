use eframe::egui;

use crate::PLUGIN_MANAGER;
use crate::components::card::{Card, CardIcon, CardProps};
use crate::{
    components::traits::StatelessComponent,
    plugin::{Capability, Plugin},
};

pub struct PluginsTab;

pub struct PluginsTabProps {}

pub struct PluginsTabOutput {}

impl StatelessComponent for PluginsTab {
    type Props<'a> = PluginsTabProps;

    type Output = PluginsTabOutput;

    fn render(ui: &mut eframe::egui::Ui, _props: Self::Props<'_>) -> Self::Output {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        ui.set_max_width(ui.available_width() - 24.0);
                        ui.heading("Plugins");
                        ui.add_space(16.0);
                        Self::render_all_capability(ui)
                    })
                });
            });

        PluginsTabOutput {}
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
