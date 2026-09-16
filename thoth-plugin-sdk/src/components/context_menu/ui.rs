//! Rendering for [`ContextMenu`].
//!
//! Drawn inside egui's context-menu closure, which already owns opening,
//! placement and dismissal — so this is only the menu's contents: the design's
//! `.menu` box, its `.menu button` rows and its `.sep` rules.

use crate::theme::{FIELD_HEIGHT, FONT_CONTROL, RADIUS_CHIP, ThemeColors, with_alpha};

use super::{ContextMenu, ContextMenuItem};

/// Side padding inside a row — design `.menu button{padding:0 8px}`.
const ROW_PAD_X: f32 = 8.0;
/// Gap between a row's icon and its label — design `.menu button{gap:8px}`.
const ROW_GAP: f32 = 8.0;
/// Inset of a separator from the menu's edges — design `.sep{margin:4px 6px}`.
const SEP_INSET: f32 = 6.0;
/// A disabled row's text, as a fraction of normal.
const DISABLED_ALPHA: u8 = 110;

impl ContextMenu {
    /// Draw the menu, returning the index of the entry chosen.
    ///
    /// Choosing an entry closes the menu, so a caller does not have to.
    pub fn show(&self, ui: &mut egui::Ui) -> Option<usize> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        ui.set_min_width(self.min_width);
        ui.spacing_mut().item_spacing.y = 0.0;

        let mut picked = None;
        for (index, item) in self.items.iter().enumerate() {
            if item.separator {
                separator(ui, &colors);
                continue;
            }
            if self.row(ui, item, &colors) {
                picked = Some(index);
            }
        }

        if picked.is_some() {
            ui.close();
        }
        picked
    }

    /// One entry. Returns whether it was chosen.
    fn row(&self, ui: &mut egui::Ui, item: &ContextMenuItem, colors: &ThemeColors) -> bool {
        let width = ui.available_width().max(self.min_width);
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(width, FIELD_HEIGHT),
            // A disabled entry is shown so the menu keeps its shape, but it
            // does not respond.
            if item.disabled {
                egui::Sense::hover()
            } else {
                egui::Sense::click()
            },
        );

        if response.hovered() && !item.disabled {
            ui.painter()
                .rect_filled(rect, RADIUS_CHIP, colors.surface);
        }

        // A checked entry takes the accent, matching the view switcher's
        // current-choice treatment.
        let text_color = if item.disabled {
            with_alpha(colors.fg_muted, DISABLED_ALPHA)
        } else if item.checked {
            colors.accent
        } else {
            colors.fg
        };

        let mut x = rect.left() + ROW_PAD_X;
        if let Some(icon) = item.icon.as_deref() {
            ui.painter().text(
                egui::pos2(x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                icon,
                egui::FontId::proportional(FONT_CONTROL),
                text_color,
            );
            x += FONT_CONTROL + ROW_GAP;
        }

        ui.painter().text(
            egui::pos2(x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            &item.label,
            egui::FontId::proportional(FONT_CONTROL),
            text_color,
        );

        // The shortcut is a reminder, so it sits right-aligned and quiet.
        if let Some(shortcut) = item.shortcut.as_deref() {
            ui.painter().text(
                egui::pos2(rect.right() - ROW_PAD_X, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                shortcut,
                egui::FontId::monospace(FONT_CONTROL - 1.0),
                with_alpha(colors.fg_muted, DISABLED_ALPHA),
            );
        } else if item.checked {
            ui.painter().text(
                egui::pos2(rect.right() - ROW_PAD_X, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                "✓",
                egui::FontId::proportional(FONT_CONTROL),
                colors.accent,
            );
        }

        response.clicked()
    }
}

fn separator(ui: &mut egui::Ui, colors: &ThemeColors) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 9.0), egui::Sense::hover());
    let y = rect.center().y;
    ui.painter().hline(
        (rect.left() + SEP_INSET)..=(rect.right() - SEP_INSET),
        y,
        egui::Stroke::new(1.0, with_alpha(colors.surface_raised, 87)),
    );
}
