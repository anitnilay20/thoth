use egui::{Align, Color32, Layout, TextFormat, text::LayoutJob};

use crate::components::IconButton;
use crate::render_node::UiEvent;
use crate::theme::{
    FONT_CONTROL, ICON_CONTROL, RADIUS_CHIP, ThemeColors, phosphor_font_id, with_alpha,
};

use super::Tabs;

/// Gap between a tab's leading icon and its label — design `.tabs .tab{gap:7px}`.
const ICON_GAP: f32 = 7.0;
/// Horizontal padding inside a tab — design `.tabs .tab{padding:0 12px}`.
const PADDING_X: f32 = 12.0;
/// Gap between tabs — design `.rtabs{gap:2px}`.
const TAB_GAP: f32 = 2.0;
/// Gap between the right-aligned trailing actions — design `.tabs .tacts{gap:1px}`.
const ACTION_GAP: f32 = 1.0;
/// The active tab's underline — design `.rtab.active::after`
/// (`left:8px;right:8px;bottom:0;height:2px;border-radius:2px`). It *is* the whole
/// active treatment: there is no pill or filled background behind an active tab.
const UNDERLINE_H: f32 = 2.0;
/// …inset from each side of the tab.
const UNDERLINE_INSET: f32 = 8.0;
/// …and its own (tiny) corner radius, below the shared ladder.
const UNDERLINE_RADIUS: f32 = 2.0;
/// Hairline rule along the strip's bottom edge — design
/// `.rtabs{box-shadow:inset 0 -1px 0 surface1@26%}`.
const STRIP_RULE_ALPHA: u8 = 66; // 26% of 255
/// Translucent hover wash behind an icon-only tab. The design sheet has no
/// icon-only tab, so this keeps the established soft-surface hover.
const ICON_HOVER_ALPHA: u8 = 40;

impl Tabs {
    /// Lay out one tab's icon + label. The *label* colour is left as
    /// [`Color32::PLACEHOLDER`] so the resolved state colour can be applied at
    /// paint time, once hover is known; the icon carries its own colour because
    /// the active tab tints only the glyph (design `.rtab.active i{color:mauve}`
    /// while `.rtab.active` keeps the label at `--text`).
    fn tab_job(
        icon: Option<&str>,
        label: &str,
        label_font: egui::FontId,
        icon_px: f32,
        icon_color: Color32,
    ) -> LayoutJob {
        let mut job = LayoutJob::default();
        let mut gap = 0.0;
        if let Some(glyph) = icon {
            job.append(
                glyph,
                0.0,
                TextFormat {
                    font_id: phosphor_font_id(icon_px),
                    color: icon_color,
                    valign: Align::Center,
                    ..Default::default()
                },
            );
            gap = ICON_GAP;
        }
        job.append(
            label,
            gap,
            TextFormat {
                // Design `.rtab{font-weight:500}` — a real medium face rather than
                // a double-drawn regular one. Resolved by the caller, which has the
                // context needed to fall back when no Medium family is registered.
                font_id: label_font,
                color: Color32::PLACEHOLDER,
                valign: Align::Center,
                ..Default::default()
            },
        );
        job
    }

    /// Render the tab header (with optional per-tab icons and right-aligned
    /// actions) and the selected panel.
    ///
    /// Emits a `"change"` event (id = the tabs id, value = the selected header
    /// label) when the active tab changes, and a `"click"` event for each action
    /// (id = the action id). The selected index is kept in egui memory.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        use crate::components::Size;

        let colors = ThemeColors::from_ctx(ui.ctx());
        // Derived from the `ui` (not a global `Id::new`) so two tab bars sharing
        // a string id — e.g. the same plugin open in two tabs — keep independent
        // selection state instead of colliding.
        let state_id = ui.make_persistent_id(("sdk_tabs", &self.id));
        let prev: usize = ui.ctx().data(|d| d.get_temp(state_id).unwrap_or(0));
        let mut selected = prev.min(self.headers.len().saturating_sub(1));

        // Size metrics: (strip height, label px, icon glyph px, icon-only cell).
        // Medium is the design sheet's `.tabs` (40px strip, 12.5px label, 15px
        // icon); Small is the compact strip the handoff tightens to 30px.
        let (strip_h, font_size, icon_px, icon_cell) = match self.size {
            Size::Small => (30.0, 11.5, ICON_CONTROL, egui::vec2(28.0, 24.0)),
            Size::Medium => (40.0, FONT_CONTROL, ICON_CONTROL, egui::vec2(34.0, 30.0)),
            Size::Large => (48.0, 14.0, 17.0, egui::vec2(40.0, 36.0)),
        };

        let content_gap = self.content_gap.unwrap_or(0.0).round() as i8;
        // The strip carries no fill and no rounding of its own — it sits directly
        // on the enclosing panel, separated from the content by a hairline rule
        // (design `.rtabs{box-shadow:inset 0 -1px 0 surface1@26%}` in
        // app-mockup.html, which is the in-app Tabs surface). Filling and rounding
        // it here would inset the tab content from its container.
        egui::Frame::new()
            .outer_margin(egui::Margin {
                left: 0,
                right: 0,
                top: 0,
                bottom: content_gap,
            })
            .inner_margin(egui::Margin {
                left: 8,
                right: 8,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_height(strip_h);
                let frame_bottom = ui.max_rect().max.y;
                // Hairline rule along the strip's bottom edge, full-bleed under
                // the 8px side padding.
                let full = ui.max_rect().expand2(egui::vec2(8.0, 0.0));
                ui.painter().hline(
                    full.x_range(),
                    frame_bottom - 0.5,
                    egui::Stroke::new(1.0, with_alpha(colors.surface_raised, STRIP_RULE_ALPHA)),
                );

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = TAB_GAP;

                    for (i, header) in self.headers.iter().enumerate() {
                        let is_active = i == selected;
                        let icon = self.icons.get(i).filter(|s| !s.is_empty());

                        // Icon-only (label as tooltip) when `icon_only` is set or
                        // the header is empty — a frameless cell with a glyph.
                        // Otherwise a full-height icon + label tab.
                        let resp = if let Some(glyph) =
                            icon.filter(|_| self.icon_only || header.is_empty())
                        {
                            let (rect, resp) =
                                ui.allocate_exact_size(icon_cell, egui::Sense::click());
                            if resp.hovered() {
                                ui.painter().rect_filled(
                                    rect,
                                    RADIUS_CHIP,
                                    with_alpha(colors.surface_raised, ICON_HOVER_ALPHA),
                                );
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            // Mauve when active, matching the labelled tab's glyph.
                            let icon_color = if is_active {
                                colors.accent
                            } else if resp.hovered() {
                                colors.fg
                            } else {
                                colors.fg_muted
                            };
                            if ui.is_rect_visible(rect) {
                                ui.painter().text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    glyph,
                                    phosphor_font_id(icon_px),
                                    icon_color,
                                );
                            }
                            // Header as tooltip (skip when empty).
                            let resp = if header.is_empty() {
                                resp
                            } else {
                                crate::theme::hover_text(resp, header.as_str())
                            };
                            if resp.clicked() && !is_active {
                                selected = i;
                            }
                            resp
                        } else {
                            // Design `.rtab`: bare text (no pill, no fill),
                            // `fg-muted` at rest, `fg` on hover *and* when active —
                            // the active tab is distinguished by its mauve icon and
                            // underline, not by a different label colour. The tab
                            // fills the strip's full height so the underline lands
                            // on the strip's bottom edge.
                            //
                            // Hover has to be resolved before layout because the
                            // icon's colour is baked into the job, so this measures
                            // the same rect the response will occupy.
                            let label_font = crate::theme::medium_font_id(ui.ctx(), font_size);
                            let probe = ui.painter().layout_job(Self::tab_job(
                                icon.map(String::as_str),
                                header,
                                label_font.clone(),
                                icon_px,
                                Color32::PLACEHOLDER,
                            ));
                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(probe.size().x + PADDING_X * 2.0, strip_h),
                                egui::Sense::click(),
                            );
                            let color = if is_active || resp.hovered() {
                                colors.fg
                            } else {
                                colors.fg_muted
                            };
                            let galley = ui.painter().layout_job(Self::tab_job(
                                icon.map(String::as_str),
                                header,
                                label_font,
                                icon_px,
                                if is_active { colors.accent } else { color },
                            ));
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if ui.is_rect_visible(rect) {
                                let pos = rect.center() - galley.rect.center().to_vec2();
                                // Real weight 500 — the label is laid out in the
                                // medium family, so no double-draw is needed.
                                ui.painter().galley(pos, galley, color);
                            }
                            if resp.clicked() && !is_active {
                                selected = i;
                            }
                            resp
                        };

                        // Active underline, pinned to the strip's bottom edge and
                        // inset from both sides of the tab.
                        if is_active {
                            let bar = egui::Rect::from_min_max(
                                egui::pos2(
                                    resp.rect.left() + UNDERLINE_INSET,
                                    frame_bottom - UNDERLINE_H,
                                ),
                                egui::pos2(resp.rect.right() - UNDERLINE_INSET, frame_bottom),
                            );
                            ui.painter()
                                .rect_filled(bar, UNDERLINE_RADIUS, colors.accent);
                        }
                    }

                    if !self.actions.is_empty() {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = ACTION_GAP;
                            for action in self.actions.iter().rev() {
                                let hit = ui
                                    .add(
                                        IconButton::builder()
                                            .icon(action.icon.as_str())
                                            .maybe_tooltip(action.tooltip.as_deref())
                                            .frame(false)
                                            .build(),
                                    )
                                    .clicked();
                                if hit {
                                    events.push(UiEvent {
                                        id: action.id.clone(),
                                        kind: "click".to_string(),
                                        value: String::new(),
                                    });
                                }
                            }
                        });
                    }
                });
            });

        if selected != prev {
            let label = self.headers.get(selected).cloned().unwrap_or_default();
            events.push(UiEvent {
                id: self.id.clone(),
                kind: "change".to_string(),
                value: label,
            });
        }
        ui.ctx().data_mut(|d| d.insert_temp(state_id, selected));

        // The selected child is rendered flush: no frame, no padding, no rounding.
        // In the app the tab strip sits *inside* an already-floating panel (the
        // seshat results pane, a dock leaf), and that panel owns the fill, edge and
        // corners — see `.rtabs` in app-mockup.html, where the content below the
        // strip runs edge to edge. Wrapping it here would inset every tab body,
        // which reads as an unwanted margin around content like `DataView`.
        if let Some(child) = self.children.get_mut(selected) {
            child.show(ui, events);
        }
    }
}
