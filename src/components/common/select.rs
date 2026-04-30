use eframe::egui;

use crate::components::traits::StatelessComponent;
use crate::theme::{ThemeColors, phosphor_font_id};

pub struct SelectOption {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum SelectSize {
    #[default]
    Default,
    Small,
}

pub struct SelectProps<'a> {
    /// Stable ID salt — must be unique per component instance on screen.
    pub id_salt: &'a str,
    /// Currently selected value (matched against `SelectOption::value`).
    pub value: &'a str,
    pub options: &'a [SelectOption],
    /// Optional static prefix shown before the selected label, e.g. `"Sort: "`.
    pub prefix_label: Option<&'a str>,
    pub size: SelectSize,
}

pub struct SelectOutput {
    /// New value if the user picked a different option, otherwise `None`.
    pub changed: Option<String>,
}

pub struct Select;

impl StatelessComponent for Select {
    type Props<'a> = SelectProps<'a>;
    type Output = SelectOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        let (trigger_h, font_size) = match props.size {
            SelectSize::Small => (22.0_f32, 10.0_f32),
            SelectSize::Default => (26.0_f32, 11.0_f32),
        };

        let id = egui::Id::new(props.id_salt);
        let mut is_open: bool = ui.ctx().data(|d| d.get_temp(id).unwrap_or(false));

        let selected_label = props
            .options
            .iter()
            .find(|o| o.value.as_str() == props.value)
            .map(|o| o.label.as_str())
            .unwrap_or(props.value);
        let display = match props.prefix_label {
            Some(pfx) => format!("{pfx}{selected_label}"),
            None => selected_label.to_string(),
        };

        // ── Trigger ───────────────────────────────────────────────────────────
        let avail_w = ui.available_width();
        let (trigger_rect, trigger_resp) =
            ui.allocate_exact_size(egui::vec2(avail_w, trigger_h), egui::Sense::click());

        if ui.is_rect_visible(trigger_rect) {
            let bg = if is_open || trigger_resp.hovered() {
                colors.surface_raised
            } else {
                colors.surface
            };
            ui.painter().rect_filled(trigger_rect, 4.0, bg);

            // Label
            ui.painter().text(
                egui::pos2(trigger_rect.min.x + 8.0, trigger_rect.center().y),
                egui::Align2::LEFT_CENTER,
                &display,
                egui::FontId::proportional(font_size),
                colors.fg,
            );

            // Caret
            ui.painter().text(
                egui::pos2(trigger_rect.max.x - 8.0, trigger_rect.center().y),
                egui::Align2::RIGHT_CENTER,
                egui_phosphor::regular::CARET_DOWN,
                phosphor_font_id(font_size - 1.0),
                if is_open { colors.fg } else { colors.fg_muted },
            );
        }

        if trigger_resp.clicked() {
            is_open = !is_open;
            let val = is_open;
            ui.ctx().data_mut(|d| d.insert_temp(id, val));
        }

        if trigger_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // ── Dropdown ──────────────────────────────────────────────────────────
        let mut changed: Option<String> = None;

        if is_open {
            let inner_pad = 4_i8;
            let inner_padf = inner_pad as f32;
            let item_h = trigger_h;
            let max_visible = 8_usize;
            let scroll_h =
                (item_h * max_visible.min(props.options.len()) as f32) + inner_padf * 2.0;

            let area_resp = egui::Area::new(id.with("_area"))
                .order(egui::Order::Foreground)
                .fixed_pos(trigger_rect.left_bottom() + egui::vec2(0.0, 3.0))
                .constrain(true)
                .interactable(true)
                .show(ui.ctx(), |ui| {
                    egui::Frame::NONE
                        .fill(colors.bg_panel)
                        .stroke(egui::Stroke::new(1.0, colors.surface))
                        .corner_radius(4)
                        .inner_margin(egui::Margin::same(inner_pad))
                        .show(ui, |ui| {
                            let popup_w = trigger_rect.width() - inner_padf * 2.0;
                            ui.set_min_width(popup_w);
                            ui.set_max_width(popup_w);

                            egui::ScrollArea::vertical()
                                .max_height(scroll_h)
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    ui.set_min_width(popup_w);
                                    for opt in props.options {
                                        let is_sel = opt.value.as_str() == props.value;
                                        let item_w = ui.available_width();
                                        let (item_rect, item_resp) = ui.allocate_exact_size(
                                            egui::vec2(item_w, item_h),
                                            egui::Sense::click(),
                                        );

                                        if ui.is_rect_visible(item_rect) {
                                            let bg = if item_resp.hovered() {
                                                Some(colors.sidebar_hover)
                                            } else if is_sel {
                                                Some(colors.surface_active)
                                            } else {
                                                None
                                            };
                                            if let Some(bg) = bg {
                                                ui.painter().rect_filled(item_rect, 3.0, bg);
                                            }
                                            ui.painter().text(
                                                egui::pos2(
                                                    item_rect.min.x + 8.0,
                                                    item_rect.center().y,
                                                ),
                                                egui::Align2::LEFT_CENTER,
                                                &opt.label,
                                                egui::FontId::proportional(font_size),
                                                colors.fg,
                                            );
                                            if is_sel {
                                                ui.painter().text(
                                                    egui::pos2(
                                                        item_rect.max.x - 8.0,
                                                        item_rect.center().y,
                                                    ),
                                                    egui::Align2::RIGHT_CENTER,
                                                    egui_phosphor::regular::CHECK,
                                                    phosphor_font_id(font_size),
                                                    colors.accent,
                                                );
                                            }
                                            if item_resp.hovered() {
                                                ui.ctx().set_cursor_icon(
                                                    egui::CursorIcon::PointingHand,
                                                );
                                            }
                                        }

                                        if item_resp.clicked() {
                                            changed = Some(opt.value.clone());
                                            ui.ctx().data_mut(|d| d.insert_temp::<bool>(id, false));
                                        }
                                    }
                                });
                        });
                });

            // Close on Escape or click outside the popup and trigger
            let escape = ui.ctx().input(|i| i.key_pressed(egui::Key::Escape));
            let interact_pos = ui
                .ctx()
                .input(|i| i.pointer.interact_pos())
                .unwrap_or_default();
            let click_outside =
                area_resp.response.clicked_elsewhere() && !trigger_rect.contains(interact_pos);

            if escape || click_outside {
                ui.ctx().data_mut(|d| d.insert_temp::<bool>(id, false));
            }
        }

        SelectOutput { changed }
    }
}
