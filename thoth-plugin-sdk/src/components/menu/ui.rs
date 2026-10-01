//! Rendering for [`Menu`] — the design's `.select` trigger with a `.menu`
//! sheet under it.
//!
//! Both halves already exist: the trigger is the one [`Select`] draws, and the
//! sheet is a [`ContextMenu`] in the popover [`Select`] drops. This is the
//! composition, so a menu and a select cannot drift apart in how they look.

use crate::components::select::ui::{paint_trigger, show_popover};
use crate::theme::{RADIUS_CHIP, ThemeColors};

use super::Menu;

impl Menu {
    /// Draw the trigger and, while it is open, the sheet.
    ///
    /// The returned [`egui::InnerResponse::inner`] is the index of the entry
    /// chosen this frame, and choosing one closes the menu.
    pub fn show(&self, ui: &mut egui::Ui) -> egui::InnerResponse<Option<usize>> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let (font_size, trigger_h) = self.size.field_metrics();
        // From the `ui`, not a global id: the same menu in two tabs must not
        // share one open flag, nor trip egui's id-clash check.
        let id = ui.make_persistent_id((self.id.as_str(), "menu"));
        let mut open: bool = ui.ctx().data(|d| d.get_temp(id).unwrap_or(false));

        let width = self.width.unwrap_or_else(|| ui.available_width());
        let (trigger_rect, trigger) = ui
            .add_enabled_ui(!self.disabled, |ui| {
                paint_trigger(
                    ui,
                    &colors,
                    egui::vec2(width, trigger_h),
                    font_size,
                    &self.label,
                    open,
                    self.icon.as_deref().map(|i| (i, colors.fg_muted)),
                    None,
                )
            })
            .inner;

        if let Some(hover) = self.hover_text.as_deref() {
            crate::theme::hover_text(trigger.clone(), hover);
        }
        if trigger.clicked() {
            open = !open;
        }

        let mut picked = None;
        if open {
            let (sheet, inner) = show_popover(
                ui,
                id.with("_sheet"),
                trigger_rect,
                &colors,
                Some(self.min_width),
                |ui, _| {
                    crate::components::ContextMenu::builder()
                        .items(self.items.clone())
                        .min_width(self.min_width)
                        .build()
                        .show(ui)
                },
            );
            picked = inner;

            // Closed by choosing, by Escape, or by a press that lands on
            // neither the sheet nor the trigger — the trigger's own click is
            // already a toggle, so counting it here would undo it.
            let pressed_away = ui.input(|i| i.pointer.any_pressed())
                && !sheet.contains_pointer()
                && !trigger.contains_pointer();
            if picked.is_some() || pressed_away || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                open = false;
            }
        }

        ui.ctx().data_mut(|d| d.insert_temp(id, open));
        crate::theme::paint_focus_ring(ui, &trigger, RADIUS_CHIP);
        egui::InnerResponse::new(picked, trigger)
    }
}

impl egui::Widget for Menu {
    /// Convenience for `ui.add(menu)`. Renders it but **discards** the
    /// choice — use [`Menu::show`] when you need it.
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui).response
    }
}
