use egui::{Align, Layout, RichText};

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
        let colors = ThemeColors::from_ctx(ui.ctx());
        let state_id = egui::Id::new(("sdk_tabs", &self.id));
        let prev: usize = ui.ctx().data(|d| d.get_temp(state_id).unwrap_or(0));
        let mut selected = prev.min(self.headers.len().saturating_sub(1));

        egui::Frame::new()
            .fill(colors.bg_panel)
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, header) in self.headers.iter().enumerate() {
                        let is_active = i == selected;
                        let icon = self.icons.get(i).filter(|s| !s.is_empty());
                        let clicked = if let Some(glyph) = icon {
                            ui.add(
                                IconButton::builder()
                                    .icon(glyph.as_str())
                                    .tooltip(header.as_str())
                                    .selected(is_active)
                                    .build(),
                            )
                            .clicked()
                        } else {
                            let text = RichText::new(header)
                                .color(if is_active { colors.fg } else { colors.fg_muted });
                            ui.selectable_label(is_active, text).clicked()
                        };
                        if clicked {
                            selected = i;
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
