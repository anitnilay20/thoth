use egui::{Align, Layout};

use crate::components::IconButton;
use crate::render_node::UiEvent;
use crate::theme::ThemeColors;

use super::Tabs;

impl Tabs {
    /// Render the tab header (with optional per-tab icons and right-aligned
    /// actions) and the selected panel.
    ///
    /// Emits a `"change"` event (id = the tabs id, value = the selected header
    /// label) when the active tab changes, and a `"click"` event for each action
    /// (id = the action id). The selected index is kept in egui memory.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        use crate::components::{Button, ButtonColor, ButtonType, Size};
        use crate::theme::phosphor_font_id;

        let colors = ThemeColors::from_ctx(ui.ctx());
        // Derived from the `ui` (not a global `Id::new`) so two tab bars sharing
        // a string id — e.g. the same plugin open in two tabs — keep independent
        // selection state instead of colliding.
        let state_id = ui.make_persistent_id(("sdk_tabs", &self.id));
        let prev: usize = ui.ctx().data(|d| d.get_temp(state_id).unwrap_or(0));
        let mut selected = prev.min(self.headers.len().saturating_sub(1));

        // Size metrics: (strip height, text-button size, icon-only glyph px,
        // icon-only cell). Small matches the compact button/select sizing.
        let (strip_h, btn_size, icon_px, icon_cell) = match self.size {
            Size::Small => (30.0, Size::Small, 15.0, egui::vec2(28.0, 24.0)),
            Size::Medium => (40.0, Size::Medium, 18.0, egui::vec2(34.0, 30.0)),
            Size::Large => (48.0, Size::Large, 20.0, egui::vec2(40.0, 36.0)),
        };

        let content_gap = self.content_gap.unwrap_or(10.0).round() as i8;
        egui::Frame::new()
            .fill(colors.bg_panel)
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

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;

                    for (i, header) in self.headers.iter().enumerate() {
                        let is_active = i == selected;
                        let icon = self.icons.get(i).filter(|s| !s.is_empty());

                        // Icon-only (label as tooltip) when `icon_only` is set or
                        // the header is empty — a frameless cell with a glyph.
                        // Otherwise a text button, with the icon as a leading glyph.
                        let resp = if let Some(glyph) =
                            icon.filter(|_| self.icon_only || header.is_empty())
                        {
                            let (rect, resp) =
                                ui.allocate_exact_size(icon_cell, egui::Sense::click());
                            if resp.hovered() {
                                let hover_bg = egui::Color32::from_rgba_premultiplied(
                                    colors.surface_raised.r(),
                                    colors.surface_raised.g(),
                                    colors.surface_raised.b(),
                                    40,
                                );
                                ui.painter().rect_filled(rect, 4.0, hover_bg);
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            let icon_color = if is_active || resp.hovered() {
                                colors.accent
                            } else {
                                ui.style().visuals.text_color()
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
                                resp.on_hover_text(header.as_str())
                            };
                            if resp.clicked() && !is_active {
                                selected = i;
                            }
                            resp
                        } else {
                            let resp = ui.add(
                                Button::builder()
                                    .label(header.as_str())
                                    .maybe_icon(icon.cloned())
                                    .button_type(ButtonType::Text)
                                    .color(if is_active {
                                        ButtonColor::Primary
                                    } else {
                                        ButtonColor::Default
                                    })
                                    .button_size(btn_size)
                                    .build(),
                            );
                            if resp.clicked() && !is_active {
                                selected = i;
                            }
                            resp
                        };

                        // Active underline pinned to the frame bottom.
                        if is_active {
                            let bar = egui::Rect::from_min_max(
                                egui::pos2(resp.rect.left(), frame_bottom - 2.0),
                                egui::pos2(resp.rect.right(), frame_bottom),
                            );
                            ui.painter().rect_filled(bar, 0.0, colors.accent);
                        }
                    }

                    if !self.actions.is_empty() {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
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

        if let Some(child) = self.children.get_mut(selected) {
            child.show(ui, events);
        }
    }
}
