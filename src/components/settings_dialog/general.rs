use eframe::egui;

use crate::components::common::select::{Select, SelectOption, SelectProps};
use crate::components::settings_dialog::helpers::{group_rows, section_header, setting_row};
use crate::components::settings_dialog::theme_picker::{ThemePicker, ThemePickerProps};
use crate::components::traits::StatelessComponent;
use crate::settings::Settings;
use crate::theme::ThemeColors;

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

                group_rows(ui, "THEME", "general-theme", colors, |ui| {
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

                // ── Theme ────────────────────────────────────────────────────
                // group_rows(ui, "THEME", "general-theme", colors, |ui| {
                //     setting_row(
                //         ui,
                //         "Active theme",
                //         Some("Changes apply immediately."),
                //         s.theme_id != b.theme_id,
                //         None,
                //         colors,
                //         |ui| {
                //             let mut id = s.theme_id.clone();
                //             let catalog = Theme::catalog();
                //             egui::ComboBox::from_id_salt("theme_id_combo")
                //                 .width(200.0)
                //                 .selected_text(
                //                     catalog
                //                         .iter()
                //                         .find(|(tid, _, _, _)| *tid == id.as_str())
                //                         .map(|(_, name, _, _)| *name)
                //                         .unwrap_or(&id),
                //                 )
                //                 .show_ui(ui, |ui| {
                //                     let mut last_family = "";
                //                     for &(tid, name, _dark, family) in catalog {
                //                         if family != last_family {
                //                             if !last_family.is_empty() {
                //                                 ui.separator();
                //                             }
                //                             ui.label(
                //                                 RichText::new(family)
                //                                     .size(10.0)
                //                                     .color(colors.fg_muted)
                //                                     .strong(),
                //                             );
                //                             last_family = family;
                //                         }
                //                         if ui
                //                             .selectable_value(&mut id, tid.to_string(), name)
                //                             .changed()
                //                         {
                //                             events
                //                                 .push(GeneralTabEvent::ThemeId(id.clone()));
                //                         }
                //                     }
                //                 });
                //         },
                //     );
                // });

                // ── Typography ───────────────────────────────────────────────
                group_rows(ui, "TYPOGRAPHY", "general-typography", colors, |ui| {
                    setting_row(
                        ui,
                        "Font size",
                        Some("Applies to all UI text. Range: 8–24 px."),
                        s.font_size != b.font_size,
                        None,
                        colors,
                        |ui| {
                            let mut val = s.font_size;
                            if ui
                                .add(
                                    egui::Slider::new(&mut val, 8.0..=24.0)
                                        .step_by(0.5)
                                        .suffix(" px"),
                                )
                                .changed()
                            {
                                events.push(GeneralTabEvent::FontSize(val));
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
                            font_opts.push(SelectOption {
                                value: String::new(),
                                label: "System default".into(),
                            });
                            for family in &families {
                                font_opts.push(SelectOption {
                                    value: family.clone(),
                                    label: family.clone(),
                                });
                            }

                            let font_out = Select::render(
                                ui,
                                SelectProps {
                                    id_salt: "font_family_combo",
                                    value: current,
                                    options: &font_opts,
                                    prefix_label: None,
                                    size: Default::default(),
                                },
                            );
                            if let Some(new_val) = font_out.changed {
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
                group_rows(ui, "WINDOW", "general-window", colors, |ui| {
                    setting_row(
                        ui,
                        "Default width",
                        Some("Initial window width. Range: 400–7680 px."),
                        s.window.default_width != b.window.default_width,
                        None,
                        colors,
                        |ui| {
                            let mut val = s.window.default_width as i32;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut val)
                                        .range(400..=7680)
                                        .suffix(" px"),
                                )
                                .changed()
                            {
                                events.push(GeneralTabEvent::WindowWidth(val as f32));
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
                            let mut val = s.window.default_height as i32;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut val)
                                        .range(300..=4320)
                                        .suffix(" px"),
                                )
                                .changed()
                            {
                                events.push(GeneralTabEvent::WindowHeight(val as f32));
                            }
                        },
                    );
                });

                ui.add_space(24.0);
            });

        GeneralTabOutput { events }
    }
}
