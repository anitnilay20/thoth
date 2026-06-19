use eframe::egui;

use crate::{
    components::traits::StatelessComponent,
    theme::{ThemeColors, icon_rich_text},
};

pub struct InputProps<'a> {
    /// Mutable buffer — owned by the caller, mutated in place.
    pub value: &'a mut String,
    /// Ghost text shown when the field is empty.
    pub placeholder: &'a str,
    /// Optional leading phosphor icon glyph (e.g. `MAGNIFYING_GLASS`).
    pub icon: Option<&'a str>,
    /// Mask text as bullets (password fields).
    pub password: bool,
    /// Disable interaction.
    pub disabled: bool,
    /// Render as a multi-line text area.
    pub multiline: bool,
    /// Visible row count when `multiline` is true (default: 4).
    pub rows: usize,
    /// `None` fills available width; `Some(w)` fixes the width to `w`.
    pub desired_width: Option<f32>,
    /// Stable ID salt for the scroll area (multiline only).
    /// `None` derives a call-site-unique ID automatically.
    pub id_salt: Option<egui::Id>,
}

pub struct InputOutput {
    pub changed: bool,
    /// The underlying `TextEdit` response — use for focus / lost-focus checks.
    pub response: egui::Response,
}

pub struct Input;

impl StatelessComponent for Input {
    type Props<'a> = InputProps<'a>;
    type Output = InputOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        let InputProps {
            value,
            placeholder,
            icon,
            password,
            disabled,
            multiline,
            rows,
            desired_width,
            id_salt,
        } = props;

        let width = desired_width.unwrap_or(f32::INFINITY);
        let mut changed = false;
        let mut resp_cell: Option<egui::Response> = None;

        let frame_resp = egui::Frame::new()
            .fill(colors.surface)
            .stroke(egui::Stroke::new(1.0, colors.surface_raised))
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.add_enabled_ui(!disabled, |ui| {
                    if multiline {
                        let row_count = rows as f32;
                        let row_height = ui.text_style_height(&egui::TextStyle::Body);
                        let fixed_h = row_height * row_count
                            + ui.spacing().item_spacing.y * (row_count - 1.0)
                            + ui.spacing().button_padding.y * 2.0;
                        let scroll_id = id_salt.unwrap_or_else(|| ui.next_auto_id());
                        let scroll_out = egui::ScrollArea::vertical()
                            .id_salt(scroll_id)
                            .max_height(fixed_h)
                            .min_scrolled_height(fixed_h)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(value)
                                        .hint_text(placeholder)
                                        .desired_rows(rows)
                                        .desired_width(width)
                                        .frame(egui::Frame::NONE),
                                )
                            });
                        let r = scroll_out.inner;
                        changed = r.changed();
                        resp_cell = Some(r);
                    } else {
                        ui.horizontal(|ui| {
                            if let Some(glyph) = icon {
                                ui.label(icon_rich_text(glyph, 13.0).color(colors.fg_muted));
                                ui.add_space(4.0);
                            }
                            let mut edit = egui::TextEdit::singleline(value)
                                .hint_text(placeholder)
                                .desired_width(width)
                                .vertical_align(egui::Align::Center)
                                .frame(egui::Frame::NONE);
                            if password {
                                edit = edit.password(true);
                            }
                            let r = ui.add(edit);
                            changed = r.changed();
                            resp_cell = Some(r);
                        });
                    }
                });
            });

        InputOutput {
            changed,
            response: resp_cell.unwrap_or(frame_resp.response),
        }
    }
}
