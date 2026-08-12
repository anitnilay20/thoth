use eframe::egui;

use crate::components::settings_dialog::helpers::{
    group_rows, section_header, setting_row, slider_control,
};
use crate::components::settings_dialog::theme_picker::{ThemePicker, ThemePickerProps};
use crate::components::traits::StatelessComponent;
use crate::settings::Settings;
use crate::theme::ThemeColors;
use thoth_plugin_sdk::components::{NumberInput, Select, SelectOption};

#[derive(Debug, Clone)]
pub enum GeneralTabEvent {
    ThemeName(String),
    FontSize(f32),
    FontFamily(Option<String>),
    WindowWidth(f32),
    WindowHeight(f32),
}

pub struct GeneralTabOutput {
    pub events: Vec<GeneralTabEvent>,
}

pub struct GeneralTabProps<'a> {
    pub settings: &'a Settings,
    pub baseline: &'a Settings,
    pub theme_colors: &'a ThemeColors,
}

pub struct GeneralTab;

impl StatelessComponent for GeneralTab {
    type Props<'a> = GeneralTabProps<'a>;
    type Output = GeneralTabOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        let s = props.settings;
        let b = props.baseline;
        let colors = props.theme_colors;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                section_header(
                    ui,
                    egui_phosphor::regular::SLIDERS,
                    "General",
                    "App-wide preferences saved to settings.toml.",
                    colors,
                );

                group_rows(ui, "THEME", |ui| {
                    let picker_out = ThemePicker::render(
                        ui,
                        ThemePickerProps {
                            colors: props.theme_colors,
                            setting: s,
                            baseline: b,
                        },
                    );
                    for evt in picker_out.events {
                        use crate::components::settings_dialog::theme_picker::ThemePickerEvent;
                        let ThemePickerEvent::ThemeSelected(name) = evt;
                        events.push(GeneralTabEvent::ThemeName(name));
                    }
                });

                // ── Typography ───────────────────────────────────────────────
                group_rows(ui, "TYPOGRAPHY", |ui| {
                    setting_row(
                        ui,
                        "Font size",
                        Some("Applies to all UI text. Range: 8–24 px."),
                        s.font_size != b.font_size,
                        None,
                        colors,
                        |ui| {
                            if let Some(val) =
                                slider_control(ui, s.font_size as f64, 8.0, 24.0, "px")
                            {
                                // Keep the old half-point granularity.
                                let snapped = (val * 2.0).round() / 2.0;
                                events.push(GeneralTabEvent::FontSize(snapped as f32));
                            }
                        },
                    );

                    setting_row(
                        ui,
                        "Font family",
                        Some("System default is recommended."),
                        s.font_family != b.font_family,
                        None,
                        colors,
                        |ui| {
                            // Enumerate installed fonts once per settings session via egui memory cache.
                            let cache_id = egui::Id::new("system_font_families");
                            let families: Vec<String> =
                                ui.ctx().data(|d| d.get_temp(cache_id)).unwrap_or_else(|| {
                                    let list = crate::platform::list_system_font_families();
                                    ui.ctx().data_mut(|d| d.insert_temp(cache_id, list.clone()));
                                    list
                                });

                            let current = s.font_family.as_deref().unwrap_or("");

                            let mut font_opts: Vec<SelectOption> =
                                Vec::with_capacity(families.len() + 1);
                            font_opts.push(
                                SelectOption::builder()
                                    .value(String::new())
                                    .label("System default")
                                    .build(),
                            );
                            for family in &families {
                                font_opts.push(
                                    SelectOption::builder()
                                        .value(family.clone())
                                        .label(family.clone())
                                        .build(),
                                );
                            }

                            let mut select = Select::builder()
                                .id("font_family_combo")
                                .value(current.to_string())
                                .options(font_opts)
                                .build();
                            if let Some(new_val) = select.show(ui).inner.selected {
                                if new_val.is_empty() {
                                    events.push(GeneralTabEvent::FontFamily(None));
                                } else {
                                    events.push(GeneralTabEvent::FontFamily(Some(new_val)));
                                }
                            }
                        },
                    );
                });

                // ── Window ───────────────────────────────────────────────────
                group_rows(ui, "WINDOW", |ui| {
                    setting_row(
                        ui,
                        "Default width",
                        Some("Initial window width. Range: 400–7680 px."),
                        s.window.default_width != b.window.default_width,
                        None,
                        colors,
                        |ui| {
                            let mut num = NumberInput::builder()
                                .id("window_default_width")
                                .value(s.window.default_width as f64)
                                .min(400.0)
                                .max(7680.0)
                                .unit("px")
                                .build();
                            if num.show(ui).changed() {
                                events.push(GeneralTabEvent::WindowWidth(num.value as f32));
                            }
                        },
                    );

                    setting_row(
                        ui,
                        "Default height",
                        Some("Initial window height. Range: 300–4320 px."),
                        s.window.default_height != b.window.default_height,
                        None,
                        colors,
                        |ui| {
                            let mut num = NumberInput::builder()
                                .id("window_default_height")
                                .value(s.window.default_height as f64)
                                .min(300.0)
                                .max(4320.0)
                                .unit("px")
                                .build();
                            if num.show(ui).changed() {
                                events.push(GeneralTabEvent::WindowHeight(num.value as f32));
                            }
                        },
                    );
                });
            });

        GeneralTabOutput { events }
    }
}
