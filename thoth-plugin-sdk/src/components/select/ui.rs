use egui::{InnerResponse, Response};

use crate::theme::{
    RADIUS_CHIP, RADIUS_CONTROL, RADIUS_POPOVER, ThemeColors, edge_stroke, focus_stroke,
    phosphor_font_id, popover_shadow, with_alpha,
};

use super::{Select, SelectResponse};

// ── Design metrics ────────────────────────────────────────────────────────────

/// Horizontal padding inside the trigger — design `.trigger{padding:0 10px}`.
const TRIGGER_PAD_X: f32 = 10.0;
/// Gap between the trigger label and its caret — design `.trigger{gap:7px}`.
const TRIGGER_GAP: f32 = 7.0;
/// Caret glyph size — design `.trigger .car{font-size:12px}`.
const CARET_SIZE: f32 = 12.0;
/// Popover inner padding — design `.popover{padding:5px}`.
const POPOVER_PAD: i8 = 5;
/// Distance from the trigger's bottom edge to the popover — design
/// `.popover{top:calc(100% + 6px)}`.
const POPOVER_GAP: f32 = 6.0;
/// Option row height for a given field height. The design pairs a 27px `.opt`
/// with a 28px `.field`, so rows sit one point inside the trigger — derived
/// rather than pinned to 27 so `Size::Small`/`Large` still scale the popover.
fn option_height(field_h: f32) -> f32 {
    field_h - 1.0
}
/// Option row horizontal padding — design `.opt{padding:0 9px}`.
const OPT_PAD_X: f32 = 9.0;
/// Gap between an option's leading content and its trailing tick — design
/// `.opt{gap:9px}`.
const OPT_GAP: f32 = 9.0;
/// Trailing tick glyph size — design `.opt .tick{font-size:14px}`.
const TICK_SIZE: f32 = 14.0;
/// Row-hover wash — design `.opt:hover{background:text@7%}`.
const HOVER_ALPHA: u8 = 18; // 7% of 255

impl Select {
    /// Render the select, updating [`value`](Select::value) on selection.
    ///
    /// The returned [`InnerResponse::inner`] carries what happened this frame:
    /// [`SelectResponse::selected`] when the user picked an option, and
    /// [`SelectResponse::search`] when a searchable dropdown's query changed.
    /// [`InnerResponse::response`] is the trigger's response.
    pub fn show(&mut self, ui: &mut egui::Ui) -> InnerResponse<SelectResponse> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let (font_size, trigger_h) = self.size.field_metrics();

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
        let (trigger_rect, trigger_resp) = paint_trigger(
            ui,
            &colors,
            egui::vec2(trigger_w, trigger_h),
            font_size,
            &display,
            is_open,
            self.icon.as_deref().filter(|g| !g.is_empty()).map(|g| {
                let tint = self
                    .icon_color
                    .as_deref()
                    .and_then(|t| crate::theme::resolve_color(t, &colors))
                    .unwrap_or(colors.fg_muted);
                (g, tint)
            }),
        );

        if trigger_resp.clicked() {
            is_open = !is_open;
            ui.ctx().data_mut(|d| d.insert_temp(id, is_open));
            // Focus the search box the moment the popup opens.
            if is_open {
                ui.ctx().data_mut(|d| d.insert_temp(focus_id, true));
            }
        }

        // ── Dropdown ──────────────────────────────────────────────────────────
        let mut out = SelectResponse::default();

        if is_open {
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

            let opt_h = option_height(trigger_h);
            let scroll_h = opt_h * max_visible.min(filtered.len().max(1)) as f32;

            let (popover_resp, ()) = show_popover(
                ui,
                id.with("_area"),
                trigger_rect,
                &colors,
                |ui, popup_w| {
                    ui.spacing_mut().item_spacing.y = 2.0;

                    // ── Search box ─────────────────────────────────────────
                    if self.searchable {
                        let edit = egui::TextEdit::singleline(&mut query)
                            .hint_text("Search…")
                            .desired_width(popup_w)
                            .font(egui::FontId::proportional(font_size));
                        let resp = ui.add_sized([popup_w, trigger_h], edit);
                        let want_focus = ui
                            .ctx()
                            .data(|d| d.get_temp::<bool>(focus_id).unwrap_or(false));
                        if want_focus {
                            resp.request_focus();
                            ui.ctx().data_mut(|d| d.remove::<bool>(focus_id));
                        }
                        if resp.changed() {
                            out.search = Some(query.clone());
                            ui.ctx()
                                .data_mut(|d| d.insert_temp(query_id, query.clone()));
                        }
                    }

                    // ── Virtualized option list ────────────────────────────
                    egui::ScrollArea::vertical()
                        .max_height(scroll_h)
                        .auto_shrink([false, true])
                        .show_rows(ui, opt_h, filtered.len(), |ui, range| {
                            ui.set_min_width(popup_w);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for row in range {
                                let opt = &self.options[filtered[row]];
                                let is_sel = opt.value == self.value;
                                let item_w = ui.available_width();
                                let (item_rect, item_resp) = ui.allocate_exact_size(
                                    egui::vec2(item_w, opt_h),
                                    egui::Sense::click(),
                                );

                                if ui.is_rect_visible(item_rect) {
                                    // Design `.opt` has no selected fill — only a
                                    // hover wash; selection reads as mauve text.
                                    if item_resp.hovered() {
                                        ui.painter().rect_filled(
                                            item_rect,
                                            RADIUS_CHIP,
                                            with_alpha(colors.fg, HOVER_ALPHA),
                                        );
                                    }
                                    // Reserve room on the right for the ✓ on the selected row.
                                    let label_max_w = (item_rect.width()
                                        - OPT_PAD_X * 2.0
                                        - if is_sel { TICK_SIZE + OPT_GAP } else { 0.0 })
                                    .max(0.0);
                                    paint_truncated(
                                        ui.painter(),
                                        egui::pos2(
                                            item_rect.min.x + OPT_PAD_X,
                                            item_rect.center().y,
                                        ),
                                        &opt.label,
                                        egui::FontId::proportional(font_size),
                                        if is_sel { colors.accent } else { colors.fg },
                                        label_max_w,
                                    );
                                    if is_sel {
                                        ui.painter().text(
                                            egui::pos2(
                                                item_rect.max.x - OPT_PAD_X,
                                                item_rect.center().y,
                                            ),
                                            egui::Align2::RIGHT_CENTER,
                                            egui_phosphor::regular::CHECK,
                                            phosphor_font_id(TICK_SIZE),
                                            colors.accent,
                                        );
                                    }
                                    if item_resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                }

                                if item_resp.clicked() {
                                    out.selected = Some(opt.value.clone());
                                    close(ui.ctx(), id, query_id);
                                }
                            }
                        });
                },
            );

            let escape = ui.ctx().input(|i| i.key_pressed(egui::Key::Escape));
            let interact_pos = ui
                .ctx()
                .input(|i| i.pointer.interact_pos())
                .unwrap_or_default();
            // Close on a click that lands outside both the popup and the trigger
            // (clicks inside the popup — search box, items, scrollbar — are kept).
            let click_outside =
                popover_resp.clicked_elsewhere() && !trigger_rect.contains(interact_pos);
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

// ── Shared dropdown chrome ────────────────────────────────────────────────────
//
// `Select` and `MultiSelect` are the same control with a different popover body,
// so the trigger and the popover shell live here and both render through them.

/// Allocate and paint a dropdown trigger — design `.trigger`: a `surface` field
/// with `RADIUS_CONTROL` corners, a hairline `edge_stroke`, the label on the
/// left and a caret pushed to the right edge (`margin-left:auto`). While open it
/// also gets a focus ring *outside* the edge (design
/// `box-shadow: var(--edge), var(--focus)`).
///
/// Design rotates the caret 180° when open; egui can only rotate a galley about
/// its first glyph, so the down caret is swapped for an up caret instead — the
/// same picture without the off-centre pivot.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_trigger(
    ui: &mut egui::Ui,
    colors: &ThemeColors,
    size: egui::Vec2,
    font_size: f32,
    label: &str,
    is_open: bool,
    icon: Option<(&str, egui::Color32)>,
) -> (egui::Rect, Response) {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        ui.painter().rect(
            rect,
            RADIUS_CONTROL,
            colors.surface,
            edge_stroke(colors),
            egui::StrokeKind::Inside,
        );
        // Open *or* merely focused: a closed trigger still shows keyboard focus.
        if is_open || resp.has_focus() {
            ui.painter().rect_stroke(
                rect,
                RADIUS_CONTROL,
                focus_stroke(colors),
                egui::StrokeKind::Outside,
            );
        }
        // A leading glyph shifts the label right by the width it actually painted
        // plus the gap (design `.viewsel` leads with an icon before the value).
        let icon_advance = match icon {
            Some((glyph, tint)) => {
                let painted = ui.painter().text(
                    egui::pos2(rect.min.x + TRIGGER_PAD_X, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    glyph,
                    phosphor_font_id(CARET_SIZE + 2.0),
                    tint,
                );
                painted.width() + TRIGGER_GAP
            }
            None => 0.0,
        };
        // Leave room for the caret on the right so the label never runs under it.
        let label_max_w =
            (rect.width() - TRIGGER_PAD_X * 2.0 - CARET_SIZE - TRIGGER_GAP - icon_advance).max(0.0);
        paint_truncated(
            ui.painter(),
            egui::pos2(rect.min.x + TRIGGER_PAD_X + icon_advance, rect.center().y),
            label,
            egui::FontId::proportional(font_size),
            if ui.is_enabled() {
                colors.fg
            } else {
                colors.fg_faint()
            },
            label_max_w,
        );
        ui.painter().text(
            egui::pos2(rect.max.x - TRIGGER_PAD_X, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            if is_open {
                egui_phosphor::regular::CARET_UP
            } else {
                egui_phosphor::regular::CARET_DOWN
            },
            phosphor_font_id(CARET_SIZE),
            colors.fg_muted,
        );
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    (rect, resp)
}

/// Show a dropdown popover under `trigger_rect` — design `.popover`: a `mantle`
/// sheet 6px below the trigger, as wide as the trigger, with `RADIUS_POPOVER`
/// corners, 5px inner padding, a drop shadow and the hairline edge.
///
/// `add_contents` receives the usable content width (the popover width minus its
/// padding). Returns the popover area's response — for click-outside detection —
/// alongside whatever `add_contents` produced.
pub(crate) fn show_popover<R>(
    ui: &egui::Ui,
    id: egui::Id,
    trigger_rect: egui::Rect,
    colors: &ThemeColors,
    add_contents: impl FnOnce(&mut egui::Ui, f32) -> R,
) -> (Response, R) {
    let content_w = trigger_rect.width() - f32::from(POPOVER_PAD) * 2.0;

    let area = egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(trigger_rect.left_bottom() + egui::vec2(0.0, POPOVER_GAP))
        .constrain(true)
        .interactable(true)
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE
                .fill(colors.bg_panel)
                .stroke(edge_stroke(colors))
                .corner_radius(RADIUS_POPOVER)
                .shadow(popover_shadow(ui.visuals().dark_mode))
                .inner_margin(egui::Margin::same(POPOVER_PAD))
                .show(ui, |ui| {
                    ui.set_min_width(content_w);
                    ui.set_max_width(content_w);
                    add_contents(ui, content_w)
                })
                .inner
        });

    (area.response, area.inner)
}

/// Paint a single line of text at a left-centered position, truncating with an
/// ellipsis if it would exceed `max_w` (so labels never overflow their column).
pub(crate) fn paint_truncated(
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
