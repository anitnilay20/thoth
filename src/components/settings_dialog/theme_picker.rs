use eframe::egui::{self, Color32, RichText};

use crate::{
    components::{
        common::input::{Input, InputProps},
        settings_dialog::helpers::setting_row,
        traits::StatelessComponent,
    },
    settings::Settings,
    theme::{Theme, ThemeColors},
};

pub struct ThemePicker;

pub struct ThemePickerProps<'a> {
    pub setting: &'a Settings,
    pub baseline: &'a Settings,
    pub colors: &'a ThemeColors,
}

#[derive(Debug, Clone)]
pub enum ThemePickerEvent {
    ThemeSelected(String),
}

pub struct ThemePickerOutput {
    pub events: Vec<ThemePickerEvent>,
}

struct CardSwatches {
    base: Color32,
    mantle: Color32,
    surface: Color32,
    text: Color32,
    primary: Color32,
    accent: Color32,
    string: Color32,
    number: Color32,
}

impl CardSwatches {
    fn from_theme(theme: &Theme) -> Self {
        let c = theme.colors();
        Self {
            base: c.bg,
            mantle: c.bg_sunken,
            surface: c.surface,
            text: c.fg,
            primary: c.accent,
            accent: c.accent_secondary,
            string: c.syntax_string,
            number: c.syntax_number,
        }
    }
}

// Card dimensions at 1.5× the original 164×106
const CARD_W: f32 = 246.0;
const CARD_H: f32 = 159.0;
const CARD_GAP: f32 = 12.0;
const CHROME_H: f32 = 26.0;
const SAMPLE_H: f32 = 90.0;
const FOOT_H: f32 = CARD_H - CHROME_H - SAMPLE_H;

impl StatelessComponent for ThemePicker {
    type Output = ThemePickerOutput;
    type Props<'a> = ThemePickerProps<'a>;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        let colors = props.colors;
        let current_name = &props.setting.theme.name;
        let baseline_name = &props.baseline.theme.name;

        // ── Active theme row ─────────────────────────────────────────────────
        let active_sw = CardSwatches::from_theme(&props.setting.theme);
        let dirty = current_name != baseline_name;
        setting_row(ui, "Active theme", None, dirty, None, colors, |ui| {
            ui.horizontal(|ui| {
                // Accent swatch dot
                let (dot_rect, _) =
                    ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                ui.painter()
                    .circle_filled(dot_rect.center(), 7.0, active_sw.primary);
                ui.add_space(6.0);
                ui.label(
                    RichText::new(current_name)
                        .size(13.0)
                        .strong()
                        .color(colors.fg),
                );
                ui.add_space(8.0);
                // Family · dark/light meta
                let (_, is_dark, family) = Theme::catalog()
                    .iter()
                    .find(|(n, _, _)| *n == current_name.as_str())
                    .cloned()
                    .unwrap_or(("".to_string(), true, "".to_string()));
                let mode = if is_dark { "dark" } else { "light" };
                ui.label(
                    RichText::new(format!("{family} · {mode}"))
                        .size(11.0)
                        .color(colors.fg_muted),
                );
            });
        });

        // ── Filter bar ───────────────────────────────────────────────────────
        let filter_id = egui::Id::new("theme_picker_filter");
        let mode_id = egui::Id::new("theme_picker_mode");
        let mut filter: String = ui.ctx().data(|d| d.get_temp(filter_id).unwrap_or_default());
        let mut mode: u8 = ui.ctx().data(|d| d.get_temp(mode_id).unwrap_or(0u8)); // 0=All 1=Dark 2=Light

        ui.add_space(10.0);
        // Render filter bar: 16px left pad | search (fills remaining) | 8px gap | tabs | 16px right pad
        // Use right-to-left layout so tabs are allocated first (fixed size), search gets the rest.
        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Reserve space for the tab pill first so search width is computed correctly.
            // We do this by switching to right-to-left for the tabs, then adding the search.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // All / Dark / Light segmented pill — same height as the search input
                // The search frame is: 12px icon + 2×5px v-padding = ~22px content area.
                // We match that with 5px v-padding on each tab button.
                egui::Frame::new()
                    .fill(colors.surface)
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(2, 2))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                            // RTL: render Light → Dark → All
                            for (i, label) in ["Light", "Dark", "All"].iter().enumerate() {
                                let tab_i = (2 - i) as u8;
                                let active = mode == tab_i;
                                let bg = if active {
                                    colors.surface_raised
                                } else {
                                    Color32::TRANSPARENT
                                };
                                let fg = if active { colors.fg } else { colors.fg_muted };

                                let btn = egui::Frame::new()
                                    .fill(bg)
                                    .corner_radius(egui::CornerRadius::same(4))
                                    .inner_margin(egui::Margin::symmetric(10, 5))
                                    .show(ui, |ui| {
                                        ui.label(RichText::new(*label).size(11.0).color(fg));
                                    });

                                if btn.response.interact(egui::Sense::click()).clicked() {
                                    mode = tab_i;
                                }
                                if btn.response.interact(egui::Sense::hover()).hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                            }
                        });
                    });

                ui.add_space(8.0);

                // Search input fills the remaining width (left of tabs)
                Input::render(
                    ui,
                    InputProps {
                        value: &mut filter,
                        placeholder: "Filter themes…",
                        icon: Some(egui_phosphor::regular::MAGNIFYING_GLASS),
                        password: false,
                        disabled: false,
                        multiline: false,
                        rows: 1,
                        desired_width: None,
                        id_salt: None,
                    },
                );
            });
        });

        ui.ctx().data_mut(|d| {
            d.insert_temp(filter_id, filter.clone());
            d.insert_temp(mode_id, mode);
        });

        ui.add_space(12.0);

        // ── Filtered + grouped catalog ────────────────────────────────────────
        let filter_lower = filter.to_lowercase();
        let catalog = Theme::catalog();

        let filtered: Vec<(String, bool, String)> = catalog
            .iter()
            .filter(|&(name, is_dark, _)| {
                let mode_ok = match mode {
                    1 => *is_dark,
                    2 => !is_dark,
                    _ => true,
                };
                let text_ok =
                    filter_lower.is_empty() || name.to_lowercase().contains(&filter_lower);
                mode_ok && text_ok
            })
            .cloned()
            .collect();

        if filtered.is_empty() {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new(format!("No themes match \"{filter}\"."))
                        .size(12.0)
                        .color(colors.fg_muted),
                );
            });
            ui.add_space(8.0);
            return ThemePickerOutput { events };
        }

        let mut families: Vec<String> = Vec::new();
        for (_, _, family) in &filtered {
            if !families.contains(family) {
                families.push(family.clone());
            }
        }

        // Columns: fit as many CARD_W cards + gaps as available width allows
        let avail = ui.available_width() - 32.0;
        let cols = ((avail + CARD_GAP) / (CARD_W + CARD_GAP)).floor().max(1.0) as usize;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui: &mut egui::Ui| {
                for family in &families {
                    let family_themes: Vec<_> =
                        filtered.iter().filter(|(_, _, f)| f == family).collect();

                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(family)
                                .size(11.0)
                                .strong()
                                .color(colors.fg_muted),
                        );
                    });
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.vertical(|ui| {
                            for row in family_themes.chunks(cols) {
                                ui.horizontal(|ui| {
                                    for (name, is_dark, _) in row {
                                        let theme = Theme::from_name(name);
                                        let sw = CardSwatches::from_theme(&theme);
                                        let selected = *name == current_name.as_str();

                                        if render_card(ui, name, *is_dark, &sw, selected, colors) {
                                            events.push(ThemePickerEvent::ThemeSelected(
                                                name.to_string(),
                                            ));
                                        }
                                        ui.add_space(CARD_GAP);
                                    }
                                });
                                ui.add_space(CARD_GAP);
                            }
                        });
                    });

                    ui.add_space(12.0);
                }
            });

        ThemePickerOutput { events }
    }
}

fn render_card(
    ui: &mut egui::Ui,
    name: &str,
    is_dark: bool,
    sw: &CardSwatches,
    selected: bool,
    host: &ThemeColors,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(CARD_W, CARD_H), egui::Sense::click());

    if !ui.is_rect_visible(rect) {
        return resp.clicked();
    }

    let p = ui.painter();
    let radius = egui::CornerRadius::same(8);

    // Card background
    p.rect_filled(rect, radius, sw.base);

    // Border
    let border_color = if selected { host.accent } else { sw.surface };
    let border_width = if selected { 2.0 } else { 1.0 };
    p.rect_stroke(
        rect.shrink(border_width * 0.5),
        radius,
        egui::Stroke::new(border_width, border_color),
        egui::StrokeKind::Middle,
    );

    // ── Chrome bar ───────────────────────────────────────────────────────────
    let chrome_rect = egui::Rect::from_min_size(rect.min, egui::vec2(CARD_W, CHROME_H));
    p.rect_filled(
        chrome_rect,
        egui::CornerRadius {
            nw: 8,
            ne: 8,
            sw: 0,
            se: 0,
        },
        sw.mantle,
    );

    let dot_y = chrome_rect.center().y;
    for (i, &dot_c) in [
        Color32::from_rgb(0xFF, 0x5F, 0x57),
        Color32::from_rgb(0xFE, 0xBC, 0x2E),
        Color32::from_rgb(0x28, 0xC8, 0x40),
    ]
    .iter()
    .enumerate()
    {
        p.circle_filled(
            egui::pos2(rect.min.x + 10.0 + i as f32 * 13.0, dot_y),
            4.5,
            dot_c,
        );
    }

    let mode_char = if is_dark { "◐" } else { "◑" };
    p.text(
        egui::pos2(chrome_rect.max.x - 10.0, dot_y),
        egui::Align2::RIGHT_CENTER,
        mode_char,
        egui::FontId::proportional(11.0),
        Color32::from_rgba_unmultiplied(sw.text.r(), sw.text.g(), sw.text.b(), 128),
    );

    p.hline(
        egui::Rangef::new(rect.min.x, rect.max.x),
        rect.min.y + CHROME_H,
        egui::Stroke::new(1.0, sw.surface),
    );

    // ── JSON sample ──────────────────────────────────────────────────────────
    let sample_top = rect.min.y + CHROME_H + 8.0;
    let font = egui::FontId::monospace(10.0);
    let cw = 6.0_f32; // approx char width at 10px mono
    let ox = rect.min.x + 10.0;
    let line_h = 14.0;

    // { "name": "thoth" }
    for (col, offset, txt) in [
        (sw.text, 0.0, "{ "),
        (sw.primary, 2.0 * cw, "\"name\""),
        (sw.text, 8.0 * cw, ": "),
        (sw.string, 10.0 * cw, "\"thoth\""),
        (sw.text, 18.0 * cw, " }"),
    ] {
        p.text(
            egui::pos2(ox + offset, sample_top),
            egui::Align2::LEFT_TOP,
            txt,
            font.clone(),
            col,
        );
    }

    //   "version": 42,
    for (col, offset, txt) in [
        (sw.primary, 0.0, "  \"version\""),
        (sw.text, 12.0 * cw, ": "),
        (sw.number, 14.0 * cw, "42"),
        (sw.text, 16.0 * cw, ","),
    ] {
        p.text(
            egui::pos2(ox + offset, sample_top + line_h),
            egui::Align2::LEFT_TOP,
            txt,
            font.clone(),
            col,
        );
    }

    //   "dark": true/false
    let bool_txt = if is_dark { "true" } else { "false" };
    for (col, offset, txt) in [
        (sw.primary, 0.0, "  \"dark\""),
        (sw.text, 9.0 * cw, ": "),
        (sw.accent, 11.0 * cw, bool_txt),
    ] {
        p.text(
            egui::pos2(ox + offset, sample_top + line_h * 2.0),
            egui::Align2::LEFT_TOP,
            txt,
            font.clone(),
            col,
        );
    }

    //   "theme": "..."
    for (col, offset, txt) in [
        (sw.primary, 0.0, "  \"theme\""),
        (sw.text, 10.0 * cw, ": "),
        (sw.string, 12.0 * cw, "\"thoth\""),
    ] {
        p.text(
            egui::pos2(ox + offset, sample_top + line_h * 3.0),
            egui::Align2::LEFT_TOP,
            txt,
            font.clone(),
            col,
        );
    }

    // ── Footer ───────────────────────────────────────────────────────────────
    let foot_top = rect.min.y + CHROME_H + SAMPLE_H;
    let foot_rect =
        egui::Rect::from_min_size(egui::pos2(rect.min.x, foot_top), egui::vec2(CARD_W, FOOT_H));
    p.rect_filled(
        foot_rect,
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: 8,
            se: 8,
        },
        Color32::from_rgba_unmultiplied(sw.mantle.r(), sw.mantle.g(), sw.mantle.b(), 200),
    );

    // Color chips
    let chip_y = foot_top + 6.0;
    for (i, &cc) in [sw.primary, sw.accent, sw.string, sw.number, sw.surface]
        .iter()
        .enumerate()
    {
        p.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.min.x + 10.0 + i as f32 * 16.0, chip_y),
                egui::vec2(13.0, 8.0),
            ),
            egui::CornerRadius::same(2),
            cc,
        );
    }

    // Theme name
    let name_y = chip_y + 13.0;
    p.text(
        egui::pos2(rect.min.x + 10.0, name_y),
        egui::Align2::LEFT_TOP,
        name,
        egui::FontId::proportional(11.0),
        Color32::from_rgba_unmultiplied(sw.text.r(), sw.text.g(), sw.text.b(), 220),
    );

    // Checkmark badge when selected — filled circle so it's visible on any bg
    if selected {
        let badge_r = 9.0_f32;
        let badge_center = egui::pos2(rect.max.x - badge_r - 6.0, name_y + badge_r);
        p.circle_filled(badge_center, badge_r, host.accent);
        p.text(
            badge_center,
            egui::Align2::CENTER_CENTER,
            egui_phosphor::regular::CHECK,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
    }

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    resp.clicked()
}
