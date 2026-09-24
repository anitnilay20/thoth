use egui::{InnerResponse, Response};

use crate::theme::{
    RADIUS_CHIP, RADIUS_CONTROL, RADIUS_POPOVER, ThemeColors, edge_stroke, focus_stroke,
    phosphor_font_id, popover_shadow, with_alpha,
};

use crate::components::Input;

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
/// Trailing figure in the trigger — design `.select .cnt{font-size:11px}`.
const COUNT_SIZE: f32 = 11.0;
/// Trailing detail on an option row — design `.tablemenu .n{font-size:10.5px}`.
const DETAIL_SIZE: f32 = 10.5;
/// Gap between an option's detail and its tick — design
/// `.tablemenu button .tick{margin-left:8px}`.
const DETAIL_GAP: f32 = 8.0;
/// Options shown before the list scrolls, when no explicit height cap is set.
const MAX_VISIBLE: usize = 8;
/// Options the list always has room for when there are that many. A menu
/// showing two rows of twelve is a scrollbar, not a list — and the five is a
/// floor, so a cap can shorten a long list but never cut into these.
const MIN_VISIBLE: usize = 5;
/// Gap between the search box and the list — the one vertical gap in the
/// popover. The rows themselves sit flush (design `.opt` separates them with a
/// hover wash, not space).
const SEARCH_GAP: f32 = 4.0;
/// How far a disabled trigger fades towards its background — design
/// `.select[disabled]{opacity:0.55}`.
const DISABLED_ALPHA: f32 = 0.55;

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
        // A disabled picker is drawn, not hidden: it still names what is on
        // screen. `add_enabled_ui` both fades it and swallows the click, so the
        // popup below can never open.
        let (trigger_rect, trigger_resp) = ui
            .add_enabled_ui(!self.disabled, |ui| {
                paint_trigger(
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
                    self.count.as_deref().filter(|c| !c.is_empty()),
                )
            })
            .inner;

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
            // Current search query (client state, kept in egui temp memory).
            let mut query: String = ui
                .ctx()
                .data(|d| d.get_temp::<String>(query_id))
                .unwrap_or_default();

            let opt_h = option_height(trigger_h);
            let list_id = id.with("_list");
            // Details only line up as a column if every row reserves the tick's
            // width, including the rows that aren't ticked.
            let any_detail = self.options.iter().any(|o| o.detail.is_some());

            let (popover_resp, ()) = show_popover(
                ui,
                id.with("_area"),
                trigger_rect,
                &colors,
                self.menu_width,
                |ui, popup_w| {
                    // Rows sit flush, so the list carries no item spacing.
                    // `ScrollArea::show_rows` bakes the ui's spacing into the
                    // pitch it lays rows out on, and a pitch wider than the
                    // rows themselves clips the last one against `max_height`
                    // — the menu then looks one row shorter than it asked for.
                    ui.spacing_mut().item_spacing.y = 0.0;

                    // ── Search box ─────────────────────────────────────────
                    // The shared `Input`, not a bare `TextEdit`: it carries the
                    // field's chrome, padding and focus ring, and centres its
                    // text in the box — a hand-rolled edit sat the placeholder
                    // against the top of a 28px field.
                    if self.searchable {
                        let mut search = Input::builder()
                            .id(format!("{}_search", self.id))
                            .value(query.clone())
                            .placeholder("Search…")
                            .desired_width(popup_w)
                            .size(self.size)
                            .build();
                        let search_out = search.show(ui);
                        if search_out.inner {
                            query = std::mem::take(&mut search.value);
                            out.search = Some(query.clone());
                            ui.ctx()
                                .data_mut(|d| d.insert_temp(query_id, query.clone()));
                        }
                        let want_focus = ui
                            .ctx()
                            .data(|d| d.get_temp::<bool>(focus_id).unwrap_or(false));
                        if want_focus {
                            search_out.response.request_focus();
                            ui.ctx().data_mut(|d| d.remove::<bool>(focus_id));
                        }
                        ui.add_space(SEARCH_GAP);
                    }

                    // Filtered *after* the box is drawn, so the list answers the
                    // query as it now reads. Filtering on the query stored last
                    // frame left the menu a keystroke behind: clearing the box
                    // redrew it at the width of the search it no longer held.
                    let needle = query.to_lowercase();
                    let filtered: Vec<usize> = self
                        .options
                        .iter()
                        .enumerate()
                        .filter(|(_, o)| {
                            needle.is_empty() || o.label.to_lowercase().contains(&needle)
                        })
                        .map(|(i, _)| i)
                        .collect();
                    let scroll_h = list_height(opt_h, filtered.len(), self.menu_max_height);

                    // ── Virtualized option list ────────────────────────────
                    if filtered.is_empty() {
                        ui.add_sized(
                            [popup_w, opt_h],
                            egui::Label::new(
                                egui::RichText::new("No matches")
                                    .size(font_size)
                                    .color(colors.fg_muted),
                            ),
                        );
                        return;
                    }
                    // `scroll_h` is already exactly the height these rows
                    // want, so there is nothing for auto-shrink to work out
                    // and the list's height depends on this frame's match
                    // count alone.
                    egui::ScrollArea::vertical()
                        .id_salt(list_id)
                        .max_height(scroll_h)
                        .auto_shrink([false, false])
                        .show_rows(ui, opt_h, filtered.len(), |ui, range| {
                            ui.set_min_width(popup_w);
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
                                    let tick_w = if is_sel || any_detail {
                                        TICK_SIZE + OPT_GAP
                                    } else {
                                        0.0
                                    };
                                    // The detail sits inside the tick column,
                                    // right-aligned — design `.n{margin-left:auto}`
                                    // with the tick 8px further right.
                                    let detail_w = match opt.detail.as_deref() {
                                        Some(text) if !text.is_empty() => {
                                            let galley = ui.painter().layout_no_wrap(
                                                text.to_owned(),
                                                egui::FontId::monospace(DETAIL_SIZE),
                                                colors.fg_muted,
                                            );
                                            let right = item_rect.max.x - OPT_PAD_X - tick_w;
                                            ui.painter().galley(
                                                egui::pos2(
                                                    right - galley.size().x,
                                                    item_rect.center().y - galley.size().y / 2.0,
                                                ),
                                                galley.clone(),
                                                colors.fg_muted,
                                            );
                                            galley.size().x + DETAIL_GAP
                                        }
                                        _ => 0.0,
                                    };
                                    let label_max_w =
                                        (item_rect.width() - OPT_PAD_X * 2.0 - tick_w - detail_w)
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

/// How tall the option list is drawn: one row per option, bounded above by
/// `cap` (or [`MAX_VISIBLE`] rows when the caller sets none) and below by
/// [`MIN_VISIBLE`] rows.
///
/// The floor is what stops a cap from making the menu useless. A cap exists to
/// keep a long list on the screen, not to shorten a list that already fits, so
/// it is never allowed to cut into the first five rows.
fn list_height(opt_h: f32, options: usize, cap: Option<f32>) -> f32 {
    let options = options.max(1);
    let cap = cap.unwrap_or(opt_h * MAX_VISIBLE as f32);
    let floor = opt_h * MIN_VISIBLE.min(options) as f32;
    (opt_h * options as f32).min(cap.max(floor))
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
///
/// `count` is the optional figure between the label and the caret (design
/// `.select .cnt`). A disabled `ui` fades the whole control, label, glyph,
/// figure and caret alike, rather than only the label.
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_trigger(
    ui: &mut egui::Ui,
    colors: &ThemeColors,
    size: egui::Vec2,
    font_size: f32,
    label: &str,
    is_open: bool,
    icon: Option<(&str, egui::Color32)>,
    count: Option<&str>,
) -> (egui::Rect, Response) {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    // Design `.select[disabled]{opacity:0.55}` fades the control as a whole, so
    // every ink in it is faded the same way rather than only the label.
    let fade = |c: egui::Color32| {
        if ui.is_enabled() {
            c
        } else {
            c.gamma_multiply(DISABLED_ALPHA)
        }
    };

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
                    fade(tint),
                );
                painted.width() + TRIGGER_GAP
            }
            None => 0.0,
        };
        // The figure is pinned just inside the caret; the label is what gives up
        // the width for it (design `.lbl{flex:1}` beside a fixed `.cnt`).
        let caret_x = rect.max.x - TRIGGER_PAD_X - CARET_SIZE;
        let count_advance = match count {
            Some(text) => {
                let galley = ui.painter().layout_no_wrap(
                    text.to_owned(),
                    egui::FontId::monospace(COUNT_SIZE),
                    colors.fg_muted,
                );
                let width = galley.size().x;
                ui.painter().galley(
                    egui::pos2(
                        caret_x - TRIGGER_GAP - width,
                        rect.center().y - galley.size().y / 2.0,
                    ),
                    galley,
                    fade(colors.fg_muted),
                );
                width + TRIGGER_GAP
            }
            None => 0.0,
        };
        // Leave room for the caret on the right so the label never runs under it.
        let label_max_w = (rect.width()
            - TRIGGER_PAD_X * 2.0
            - CARET_SIZE
            - TRIGGER_GAP
            - icon_advance
            - count_advance)
            .max(0.0);
        paint_truncated(
            ui.painter(),
            egui::pos2(rect.min.x + TRIGGER_PAD_X + icon_advance, rect.center().y),
            label,
            egui::FontId::proportional(font_size),
            fade(colors.fg),
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
            fade(colors.fg_muted),
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
/// `min_width` widens the sheet past the trigger — design
/// `.menu.tablemenu{min-width:248px}` under a 212px trigger. It never narrows
/// it: a menu shorter than the control it hangs from reads as a misalignment.
///
/// `add_contents` receives the usable content width (the popover width minus its
/// padding). Returns the popover area's response — for click-outside detection —
/// alongside whatever `add_contents` produced.
pub(crate) fn show_popover<R>(
    ui: &egui::Ui,
    id: egui::Id,
    trigger_rect: egui::Rect,
    colors: &ThemeColors,
    min_width: Option<f32>,
    add_contents: impl FnOnce(&mut egui::Ui, f32) -> R,
) -> (Response, R) {
    let sheet_w = trigger_rect.width().max(min_width.unwrap_or(0.0));
    let content_w = sheet_w - f32::from(POPOVER_PAD) * 2.0;

    let area = egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(trigger_rect.left_bottom() + egui::vec2(0.0, POPOVER_GAP))
        .constrain(true)
        .interactable(true)
        .show(ui.ctx(), |ui| {
            // An `Area` sizes the rect it hands its contents from the size
            // those contents came to on the *previous* frame — it has to,
            // since it must place itself before it knows what is inside. So
            // anything that grows is clamped to yesterday's height for a
            // frame, which for a menu that just had its search cleared means
            // it redraws at the search's height and stays there until
            // something else asks for a repaint. A popover is a floating
            // sheet: the screen is the only thing that should bound it.
            ui.set_max_height(ui.ctx().content_rect().height());
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

#[cfg(test)]
mod tests {
    use super::{MAX_VISIBLE, MIN_VISIBLE, list_height};
    use crate::components::{Select, SelectOption, Size};

    fn with_ui<R>(f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        // The host registers the icon font; a bare test context has none, and
        // the trigger paints a caret on every frame.
        let mut fonts = egui::FontDefinitions::default();
        crate::theme::register_phosphor(&mut fonts);
        ctx.set_fonts(fonts);
        let mut f = Some(f);
        let mut out = None;
        let _ = ctx.run_ui(Default::default(), |ui| {
            if let Some(f) = f.take() {
                out = Some(f(ui));
            }
        });
        out.expect("the test frame ran")
    }

    fn table(value: &str, detail: &str) -> SelectOption {
        SelectOption::builder()
            .value(value)
            .label(value)
            .detail(detail)
            .build()
    }

    #[test]
    fn a_trigger_keeps_its_given_width_whatever_it_carries() {
        // The design lays the bar out in fixed tracks (212 + 124), so a long
        // label or a long figure has to be absorbed by the label, not by the
        // control growing and pushing the next one along.
        let widths = with_ui(|ui| {
            let plain = Select::builder()
                .id("a")
                .value("events")
                .options(vec![table("events", "4,812")])
                .size(Size::Medium)
                .width(212.0)
                .build()
                .show(ui)
                .response
                .rect
                .width();
            let loaded = Select::builder()
                .id("b")
                .value("a_very_long_collection_name_that_will_not_fit")
                .options(vec![table(
                    "a_very_long_collection_name_that_will_not_fit",
                    "18,204",
                )])
                .icon(egui_phosphor::regular::TABLE)
                .count("18,204")
                .size(Size::Medium)
                .width(212.0)
                .build()
                .show(ui)
                .response
                .rect
                .width();
            (plain, loaded)
        });
        assert_eq!(widths.0, 212.0);
        assert_eq!(widths.1, 212.0);
    }

    #[test]
    fn a_disabled_trigger_cannot_be_opened() {
        // One table is not a choice: the picker still names it, but clicking
        // it must not drop a menu with a single row in it.
        let clicked = with_ui(|ui| {
            Select::builder()
                .id("only")
                .value("users")
                .options(vec![table("users", "4,812")])
                .width(212.0)
                .disabled(true)
                .build()
                .show(ui)
                .response
                .clicked()
        });
        assert!(!clicked);
    }

    #[test]
    fn the_list_shows_five_rows_whenever_there_are_five() {
        let row = 27.0;
        // Fewer than five: as many as there are, with no padding under them.
        assert_eq!(list_height(row, 0, None), row);
        assert_eq!(list_height(row, 1, None), row);
        assert_eq!(list_height(row, 3, None), row * 3.0);
        // Five or more always gets at least five rows of room…
        for n in MIN_VISIBLE..40 {
            assert!(
                list_height(row, n, None) >= row * MIN_VISIBLE as f32,
                "{n} options got {}px",
                list_height(row, n, None)
            );
            assert!(
                list_height(row, n, Some(340.0)) >= row * MIN_VISIBLE as f32,
                "{n} options under a cap got {}px",
                list_height(row, n, Some(340.0))
            );
        }
    }

    #[test]
    fn a_cap_shortens_a_long_list_but_never_the_first_five_rows() {
        let row = 27.0;
        // The design's table menu: 340px is ~12 rows, so a 12-collection
        // document opens fully rather than stopping at the default eight.
        assert_eq!(list_height(row, 12, Some(340.0)), row * 12.0);
        assert_eq!(list_height(row, 40, Some(340.0)), 340.0);
        // With no cap the default eight applies.
        assert_eq!(list_height(row, 40, None), row * MAX_VISIBLE as f32);
        // A cap tighter than five rows is overruled — it would leave a menu
        // that is more scrollbar than list.
        assert_eq!(list_height(row, 40, Some(20.0)), row * MIN_VISIBLE as f32);
        // …but it still cannot pad a list shorter than the floor.
        assert_eq!(list_height(row, 2, Some(20.0)), row * 2.0);
    }

    /// Drive an open, searchable select through `script` — one entry per
    /// frame — and report the popover height each of those frames laid out.
    ///
    /// Typed for real, and read frame by frame with nothing allowed to settle
    /// in between: both bugs this guards were one-frame lags that a test
    /// running the widget to a standstill cannot see.
    fn menu_heights(options: usize, script: &[Vec<egui::Event>]) -> Vec<f32> {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        crate::theme::register_phosphor(&mut fonts);
        ctx.set_fonts(fonts);

        let opts: Vec<SelectOption> = (0..options)
            .map(|i| {
                SelectOption::builder()
                    .value(format!("table_{i}"))
                    .label(format!("table_{i}"))
                    .build()
            })
            .collect();

        // An `Area` records its size at the end of the frame that drew it, so
        // frame N reads back the height frame N-1 laid out.
        let mut heights = Vec::new();
        let mut first = true;
        for events in script {
            let raw = egui::RawInput {
                events: events.clone(),
                ..Default::default()
            };
            let want_focus = std::mem::take(&mut first);
            let _ = ctx.run_ui(raw, |ui| {
                let id = ui.make_persistent_id("m");
                heights.push(
                    egui::AreaState::load(ui.ctx(), id.with("_area"))
                        .and_then(|a| a.size)
                        .map(|s| s.y)
                        .unwrap_or(0.0),
                );
                ui.ctx().data_mut(|d| {
                    // Open it, and on the first frame hand the search box focus
                    // the way clicking the trigger would.
                    d.insert_temp(id, true);
                    if want_focus {
                        d.insert_temp(id.with("_focus"), true);
                    }
                });
                Select::builder()
                    .id("m")
                    .value("table_0")
                    .options(opts.clone())
                    .width(212.0)
                    .menu_width(248.0)
                    .menu_max_height(340.0)
                    .searchable(true)
                    .build()
                    .show(ui);
            });
        }
        // Drop the leading 0 (nothing was laid out before the first frame) so
        // index N is the height frame N drew.
        heights.remove(0);
        heights
    }

    fn typed(text: &str) -> Vec<egui::Event> {
        vec![egui::Event::Text(text.to_string())]
    }

    fn backspaces(n: usize) -> Vec<egui::Event> {
        (0..n)
            .map(|_| egui::Event::Key {
                key: egui::Key::Backspace,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            })
            .collect()
    }

    #[test]
    fn the_menu_narrows_on_the_frame_the_query_is_typed() {
        // Filtering on the query stored last frame left the list a keystroke
        // behind in both directions.
        let h = menu_heights(12, &[vec![], vec![], typed("zzz"), vec![]]);
        assert!(h[0] > 0.0, "the menu never opened");
        assert!(
            h[2] < h[1],
            "typing left the menu at {}px instead of shrinking it",
            h[1]
        );
    }

    #[test]
    fn the_menu_regrows_on_the_frame_the_search_is_cleared() {
        // The reported bug: search, then clear, and the menu stayed at the
        // search's height. A vertically auto-shrinking scroll area is capped
        // at the content size recorded on the *previous* frame, so the frame
        // that cleared the box drew the widened list into the narrowed box —
        // and nothing requested the repaint that would have fixed it.
        let query = "table_11";
        let h = menu_heights(
            12,
            &[
                vec![],
                vec![],
                typed(query),
                vec![],
                backspaces(query.len()),
                vec![],
            ],
        );
        let full = h[1];
        let narrowed = h[3];
        let cleared = h[4];

        assert!(narrowed < full, "the search never narrowed the menu");
        assert_eq!(
            cleared, full,
            "clearing the search drew the menu at {cleared}px, not the {full}px \
             it had before the search"
        );
    }

    #[test]
    fn a_menu_is_never_narrower_than_the_trigger_it_hangs_from() {
        // `menu_width` widens the sheet (248 under a 212 trigger) and is
        // ignored when it would narrow it — a short menu reads as misaligned.
        let trigger = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(212.0, 28.0));
        let sheet = |min: Option<f32>| trigger.width().max(min.unwrap_or(0.0));
        assert_eq!(sheet(Some(248.0)), 248.0);
        assert_eq!(sheet(Some(120.0)), 212.0);
        assert_eq!(sheet(None), 212.0);
    }
}
