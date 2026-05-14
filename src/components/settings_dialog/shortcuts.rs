use eframe::egui;

use crate::components::settings_dialog::helpers::{group_rows, section_header, setting_row};
use crate::components::traits::StatelessComponent;
use crate::shortcuts::{KeyboardShortcuts, Shortcut};
use crate::theme::ThemeColors;

pub struct ShortcutsTab;

pub struct ShortcutsTabProps<'a> {
    pub shortcuts: &'a KeyboardShortcuts,
    pub theme_colors: &'a ThemeColors,
}

#[derive(Debug, Clone)]
pub enum ShortcutsTabEvent {}

pub struct ShortcutsTabOutput {
    pub events: Vec<ShortcutsTabEvent>,
}

impl StatelessComponent for ShortcutsTab {
    type Props<'a> = ShortcutsTabProps<'a>;
    type Output = ShortcutsTabOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let sc = props.shortcuts;
        let colors = props.theme_colors;

        // Pre-compute the widest badge so every badge gets the same width.
        let badge_width = {
            let font_id = egui::FontId::proportional(12.0);
            let all: &[&Shortcut] = &[
                &sc.open_file,
                &sc.clear_file,
                &sc.new_window,
                &sc.focus_search,
                &sc.next_match,
                &sc.prev_match,
                &sc.nav_back,
                &sc.nav_forward,
                &sc.escape,
                &sc.expand_node,
                &sc.collapse_node,
                &sc.expand_all,
                &sc.collapse_all,
                &sc.copy_key,
                &sc.copy_value,
                &sc.copy_object,
                &sc.copy_path,
                &sc.toggle_bookmark,
                &sc.open_bookmarks,
                &sc.move_up,
                &sc.move_down,
                &sc.settings,
                &sc.toggle_theme,
                &sc.toggle_profiler,
            ];
            let max_text_w = all
                .iter()
                .map(|s| {
                    let txt = s.format();
                    if txt.is_empty() {
                        return 0.0_f32;
                    }
                    ui.painter()
                        .layout_no_wrap(txt, font_id.clone(), colors.fg)
                        .size()
                        .x
                })
                .fold(0.0_f32, f32::max);
            // text width + 2×horizontal pad (8px each side)
            (max_text_w + 16.0).ceil()
        };

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                section_header(
                    ui,
                    egui_phosphor::regular::KEYBOARD,
                    "Shortcuts",
                    "Keyboard shortcuts per action.",
                    colors,
                );

                // ── File ────────────────────────────────────────────────────
                group_rows(ui, "FILE", "sc-file", colors, |ui| {
                    shortcut_row(ui, "Open file", &sc.open_file, badge_width, colors);
                    shortcut_row(ui, "Close file", &sc.clear_file, badge_width, colors);
                    shortcut_row(ui, "New window", &sc.new_window, badge_width, colors);
                });

                // ── Navigation ───────────────────────────────────────────────
                group_rows(ui, "NAVIGATION", "sc-nav", colors, |ui| {
                    shortcut_row(ui, "Focus search", &sc.focus_search, badge_width, colors);
                    shortcut_row(ui, "Next match", &sc.next_match, badge_width, colors);
                    shortcut_row(ui, "Previous match", &sc.prev_match, badge_width, colors);
                    shortcut_row(ui, "Navigate back", &sc.nav_back, badge_width, colors);
                    shortcut_row(ui, "Navigate forward", &sc.nav_forward, badge_width, colors);
                    shortcut_row(ui, "Escape / dismiss", &sc.escape, badge_width, colors);
                });

                // ── Tree ─────────────────────────────────────────────────────
                group_rows(ui, "TREE", "sc-tree", colors, |ui| {
                    shortcut_row(ui, "Expand node", &sc.expand_node, badge_width, colors);
                    shortcut_row(ui, "Collapse node", &sc.collapse_node, badge_width, colors);
                    shortcut_row(ui, "Expand all", &sc.expand_all, badge_width, colors);
                    shortcut_row(ui, "Collapse all", &sc.collapse_all, badge_width, colors);
                });

                // ── Clipboard ────────────────────────────────────────────────
                group_rows(ui, "CLIPBOARD", "sc-clip", colors, |ui| {
                    shortcut_row(ui, "Copy key", &sc.copy_key, badge_width, colors);
                    shortcut_row(ui, "Copy value", &sc.copy_value, badge_width, colors);
                    shortcut_row(ui, "Copy object", &sc.copy_object, badge_width, colors);
                    shortcut_row(ui, "Copy path", &sc.copy_path, badge_width, colors);
                });

                // ── Bookmarks ────────────────────────────────────────────────
                group_rows(ui, "BOOKMARKS", "sc-marks", colors, |ui| {
                    shortcut_row(
                        ui,
                        "Toggle bookmark",
                        &sc.toggle_bookmark,
                        badge_width,
                        colors,
                    );
                    shortcut_row(
                        ui,
                        "Open bookmarks",
                        &sc.open_bookmarks,
                        badge_width,
                        colors,
                    );
                });

                // ── Movement ────────────────────────────────────────────────
                group_rows(ui, "MOVEMENT", "sc-move", colors, |ui| {
                    shortcut_row(ui, "Move up", &sc.move_up, badge_width, colors);
                    shortcut_row(ui, "Move down", &sc.move_down, badge_width, colors);
                });

                // ── UI ───────────────────────────────────────────────────────
                group_rows(ui, "UI", "sc-ui", colors, |ui| {
                    shortcut_row(ui, "Open settings", &sc.settings, badge_width, colors);
                    shortcut_row(ui, "Toggle theme", &sc.toggle_theme, badge_width, colors);
                });

                // ── Developer ────────────────────────────────────────────────
                group_rows(ui, "DEVELOPER", "sc-dev", colors, |ui| {
                    shortcut_row(
                        ui,
                        "Toggle profiler",
                        &sc.toggle_profiler,
                        badge_width,
                        colors,
                    );
                });

                ui.add_space(24.0);
            });

        ShortcutsTabOutput { events: Vec::new() }
    }
}

/// Render a single shortcut as a `setting_row` with a fixed-width keyboard badge.
fn shortcut_row(
    ui: &mut egui::Ui,
    label: &str,
    shortcut: &Shortcut,
    badge_width: f32,
    colors: &ThemeColors,
) {
    setting_row(ui, label, None, false, None, colors, |ui| {
        kbd_badge(ui, &shortcut.format(), badge_width, colors);
    });
}

/// A pill-shaped keyboard shortcut badge with a uniform fixed width.
fn kbd_badge(ui: &mut egui::Ui, text: &str, width: f32, colors: &ThemeColors) {
    let pad_v = 4.0;
    let height = ui.text_style_height(&egui::TextStyle::Body) + pad_v * 2.0;

    if text.is_empty() {
        // Still allocate the same width so columns stay aligned.
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "—",
            egui::FontId::proportional(12.0),
            colors.fg_muted,
        );
        return;
    }

    let font_id = egui::FontId::proportional(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font_id, colors.fg);

    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());

    ui.painter().rect(
        rect,
        egui::CornerRadius::same(4),
        colors.bg_sunken,
        egui::Stroke::new(1.0, colors.surface_active),
        egui::StrokeKind::Outside,
    );

    // Centre the text inside the fixed-width pill.
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        colors.fg,
    );
}
