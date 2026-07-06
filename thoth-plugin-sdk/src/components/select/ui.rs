use egui::{InnerResponse, Response};

use crate::theme::{ThemeColors, phosphor_font_id};

use super::{Select, SelectResponse};
use crate::components::Size;

impl Select {
    /// Render the select, updating [`value`](Select::value) on selection.
    ///
    /// The returned [`InnerResponse::inner`] carries what happened this frame:
    /// [`SelectResponse::selected`] when the user picked an option, and
    /// [`SelectResponse::search`] when a searchable dropdown's query changed.
    /// [`InnerResponse::response`] is the trigger's response.
    pub fn show(&mut self, ui: &mut egui::Ui) -> InnerResponse<SelectResponse> {
        let colors = ThemeColors::from_ctx(ui.ctx());

        let (trigger_h, font_size) = match self.size {
            Size::Small => (24.0_f32, 10.0_f32),
            Size::Medium => (28.0_f32, 11.0_f32),
            Size::Large => (32.0_f32, 12.0_f32),
        };

        // Derived from the `ui` (not a global `Id::new`) so two selects sharing
        // a string id — e.g. the same plugin open in two tabs — get distinct
        // popup/query state and don't trip egui's widget-id clash detector.
        let id = ui.make_persistent_id(&self.id);
        let query_id = id.with("_query");
        let focus_id = id.with("_focus");
        let mut is_open: bool = ui.ctx().data(|d| d.get_temp(id).unwrap_or(false));

        let selected_label = self
            .options
            .iter()
            .find(|o| o.value == self.value)
            .map(|o| o.label.as_str())
            .unwrap_or(self.value.as_str());
        let display = match &self.prefix_label {
            Some(pfx) => format!("{pfx}{selected_label}"),
            None => selected_label.to_string(),
        };

        // ── Trigger ───────────────────────────────────────────────────────────
        let trigger_w = self.width.unwrap_or_else(|| ui.available_width());
        let (trigger_rect, trigger_resp) =
            ui.allocate_exact_size(egui::vec2(trigger_w, trigger_h), egui::Sense::click());

        if ui.is_rect_visible(trigger_rect) {
            let bg = if is_open || trigger_resp.hovered() {
                colors.surface_raised
            } else {
                colors.surface
            };
            ui.painter().rect_filled(trigger_rect, 4.0, bg);
            // Leave room for the caret on the right so the label never runs under it.
            let label_max_w = (trigger_rect.width() - 8.0 - 22.0).max(0.0);
            paint_truncated(
                ui.painter(),
                egui::pos2(trigger_rect.min.x + 8.0, trigger_rect.center().y),
                &display,
                egui::FontId::proportional(font_size),
                colors.fg,
                label_max_w,
            );
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
            ui.ctx().data_mut(|d| d.insert_temp(id, is_open));
            // Focus the search box the moment the popup opens.
            if is_open {
                ui.ctx().data_mut(|d| d.insert_temp(focus_id, true));
            }
        }
        if trigger_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // ── Dropdown ──────────────────────────────────────────────────────────
        let mut out = SelectResponse::default();

        if is_open {
            let inner_pad = 4_i8;
            let inner_padf = inner_pad as f32;
            let item_h = trigger_h;
            let max_visible = 8_usize;

            // Current search query (client state, kept in egui temp memory).
            let mut query: String = ui
                .ctx()
                .data(|d| d.get_temp::<String>(query_id))
                .unwrap_or_default();

            // Options matching the query (case-insensitive substring).
            let needle = query.to_lowercase();
            let filtered: Vec<usize> = self
                .options
                .iter()
                .enumerate()
                .filter(|(_, o)| needle.is_empty() || o.label.to_lowercase().contains(&needle))
                .map(|(i, _)| i)
                .collect();

            let scroll_h = (item_h * max_visible.min(filtered.len().max(1)) as f32) + inner_padf;

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
                            ui.spacing_mut().item_spacing.y = 2.0;

                            // ── Search box ─────────────────────────────────────
                            if self.searchable {
                                let edit = egui::TextEdit::singleline(&mut query)
                                    .hint_text("Search…")
                                    .desired_width(popup_w)
                                    .font(egui::FontId::proportional(font_size));
                                let resp = ui.add_sized([popup_w, item_h], edit);
                                let want_focus =
                                    ui.ctx().data(|d| d.get_temp::<bool>(focus_id).unwrap_or(false));
                                if want_focus {
                                    resp.request_focus();
                                    ui.ctx().data_mut(|d| d.remove::<bool>(focus_id));
                                }
                                if resp.changed() {
                                    out.search = Some(query.clone());
                                    ui.ctx().data_mut(|d| d.insert_temp(query_id, query.clone()));
                                }
                            }

                            // ── Virtualized option list ────────────────────────
                            egui::ScrollArea::vertical()
                                .max_height(scroll_h)
                                .auto_shrink([false, true])
                                .show_rows(ui, item_h, filtered.len(), |ui, range| {
                                    ui.set_min_width(popup_w);
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    for row in range {
                                        let opt = &self.options[filtered[row]];
                                        let is_sel = opt.value == self.value;
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
                                            // Reserve room on the right for the ✓ on the selected row.
                                            let label_max_w = (item_rect.width()
                                                - 8.0
                                                - if is_sel { 22.0 } else { 8.0 })
                                            .max(0.0);
                                            paint_truncated(
                                                ui.painter(),
                                                egui::pos2(
                                                    item_rect.min.x + 8.0,
                                                    item_rect.center().y,
                                                ),
                                                &opt.label,
                                                egui::FontId::proportional(font_size),
                                                colors.fg,
                                                label_max_w,
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
                                            out.selected = Some(opt.value.clone());
                                            close(ui.ctx(), id, query_id);
                                        }
                                    }
                                });
                        });
                });

            let escape = ui.ctx().input(|i| i.key_pressed(egui::Key::Escape));
            let interact_pos = ui
                .ctx()
                .input(|i| i.pointer.interact_pos())
                .unwrap_or_default();
            // Close on a click that lands outside both the popup and the trigger
            // (clicks inside the popup — search box, items, scrollbar — are kept).
            let click_outside =
                area_resp.response.clicked_elsewhere() && !trigger_rect.contains(interact_pos);
            if escape || click_outside {
                close(ui.ctx(), id, query_id);
            }
        }

        if let Some(new_value) = &out.selected {
            self.value = new_value.clone();
        }
        InnerResponse::new(out, trigger_resp)
    }
}

/// Close the popup and clear its search query, so it reopens fresh.
fn close(ctx: &egui::Context, id: egui::Id, query_id: egui::Id) {
    ctx.data_mut(|d| {
        d.insert_temp::<bool>(id, false);
        d.remove::<String>(query_id);
    });
}

/// Paint a single line of text at a left-centered position, truncating with an
/// ellipsis if it would exceed `max_w` (so labels never overflow their column).
fn paint_truncated(
    painter: &egui::Painter,
    left_center: egui::Pos2,
    text: &str,
    font_id: egui::FontId,
    color: egui::Color32,
    max_w: f32,
) {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat {
            font_id,
            color,
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_width: max_w,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let galley = painter.layout_job(job);
    let pos = egui::pos2(left_center.x, left_center.y - galley.size().y / 2.0);
    painter.galley(pos, galley, color);
}

impl egui::Widget for Select {
    /// Convenience for `ui.add(select)` — renders but **discards** the
    /// selection. Use [`Select::show`] to capture it.
    fn ui(mut self, ui: &mut egui::Ui) -> Response {
        self.show(ui).response
    }
}
