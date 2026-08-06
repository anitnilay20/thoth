use eframe::egui;

use crate::components::settings_dialog::helpers::{group_rows, section_header, setting_row};
use crate::components::traits::StatelessComponent;
use crate::shortcuts::{KeyboardShortcuts, Shortcut};
use crate::theme::{ThemeColors, edge_stroke};
use thoth_plugin_sdk::theme::RADIUS_CONTROL;

/// Shortcut badge height — design `.pfield{height:26px}`.
const BADGE_H: f32 = 26.0;
/// …and its label size — design `.pfield{font-size:12px}`.
const BADGE_FONT: f32 = 12.0;
/// Horizontal padding inside a badge — design `.pfield{padding:0 10px}`.
const BADGE_PAD_H: f32 = 10.0;

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
            let font_id = egui::FontId::proportional(BADGE_FONT);
            let all: &[&Shortcut] = &[
                &sc.open_file,
                &sc.new_window,
                &sc.close_tab,
                &sc.new_tab,
                &sc.next_tab,
                &sc.prev_tab,
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
            // text width + the design's 10px padding on each side
            (max_text_w + BADGE_PAD_H * 2.0).ceil()
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
                group_rows(ui, "FILE", |ui| {
                    shortcut_row(ui, "Open file", &sc.open_file, badge_width, colors);
                    shortcut_row(ui, "New window", &sc.new_window, badge_width, colors);
                });

                // ── Tabs ─────────────────────────────────────────────────────
                group_rows(ui, "TABS", |ui| {
                    shortcut_row(ui, "New tab", &sc.new_tab, badge_width, colors);
                    shortcut_row(ui, "Close tab", &sc.close_tab, badge_width, colors);
                    shortcut_row(ui, "Next tab", &sc.next_tab, badge_width, colors);
                    shortcut_row(ui, "Previous tab", &sc.prev_tab, badge_width, colors);
                    static_shortcut_row(
                        ui,
                        "Switch to tab 1–9",
                        if cfg!(target_os = "macos") {
                            "⌘1 – ⌘9"
                        } else {
                            "Ctrl+1 – Ctrl+9"
                        },
                        badge_width,
                        colors,
                    );
                });

                // ── Navigation ───────────────────────────────────────────────
                group_rows(ui, "NAVIGATION", |ui| {
                    shortcut_row(ui, "Focus search", &sc.focus_search, badge_width, colors);
                    shortcut_row(ui, "Next match", &sc.next_match, badge_width, colors);
                    shortcut_row(ui, "Previous match", &sc.prev_match, badge_width, colors);
                    shortcut_row(ui, "Navigate back", &sc.nav_back, badge_width, colors);
                    shortcut_row(ui, "Navigate forward", &sc.nav_forward, badge_width, colors);
                    shortcut_row(ui, "Escape / dismiss", &sc.escape, badge_width, colors);
                });

                // ── Tree ─────────────────────────────────────────────────────
                group_rows(ui, "TREE", |ui| {
                    shortcut_row(ui, "Expand node", &sc.expand_node, badge_width, colors);
                    shortcut_row(ui, "Collapse node", &sc.collapse_node, badge_width, colors);
                    shortcut_row(ui, "Expand all", &sc.expand_all, badge_width, colors);
                    shortcut_row(ui, "Collapse all", &sc.collapse_all, badge_width, colors);
                });

                // ── Clipboard ────────────────────────────────────────────────
                group_rows(ui, "CLIPBOARD", |ui| {
                    shortcut_row(ui, "Copy key", &sc.copy_key, badge_width, colors);
                    shortcut_row(ui, "Copy value", &sc.copy_value, badge_width, colors);
                    shortcut_row(ui, "Copy object", &sc.copy_object, badge_width, colors);
                    shortcut_row(ui, "Copy path", &sc.copy_path, badge_width, colors);
                });

                // ── Bookmarks ────────────────────────────────────────────────
                group_rows(ui, "BOOKMARKS", |ui| {
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
                group_rows(ui, "MOVEMENT", |ui| {
                    shortcut_row(ui, "Move up", &sc.move_up, badge_width, colors);
                    shortcut_row(ui, "Move down", &sc.move_down, badge_width, colors);
                });

                // ── UI ───────────────────────────────────────────────────────
                group_rows(ui, "UI", |ui| {
                    shortcut_row(ui, "Open settings", &sc.settings, badge_width, colors);
                    shortcut_row(ui, "Toggle theme", &sc.toggle_theme, badge_width, colors);
                });

                // ── Developer ────────────────────────────────────────────────
                group_rows(ui, "DEVELOPER", |ui| {
                    shortcut_row(
                        ui,
                        "Toggle profiler",
                        &sc.toggle_profiler,
                        badge_width,
                        colors,
                    );
                });
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

/// Render a shortcut row with a literal string badge (for hardcoded shortcuts like ⌘1–9).
fn static_shortcut_row(
    ui: &mut egui::Ui,
    label: &str,
    text: &str,
    badge_width: f32,
    colors: &ThemeColors,
) {
    setting_row(ui, label, None, false, None, colors, |ui| {
        kbd_badge(ui, text, badge_width, colors);
    });
}

/// A keyboard shortcut badge with a uniform fixed width, styled as the design's
/// read-only value field — `.pfield`: surface fill, hairline edge, mono label.
fn kbd_badge(ui: &mut egui::Ui, text: &str, width: f32, colors: &ThemeColors) {
    if text.is_empty() {
        // Still allocate the same width so columns stay aligned.
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, BADGE_H), egui::Sense::hover());
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "—",
            egui::FontId::proportional(BADGE_FONT),
            colors.fg_faint(),
        );
        return;
    }

    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        egui::FontId::proportional(BADGE_FONT),
        colors.fg_subtle(),
    );

    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, BADGE_H), egui::Sense::hover());
    let radius = egui::CornerRadius::from(RADIUS_CONTROL);
    ui.painter().rect(
        rect,
        radius,
        colors.surface,
        edge_stroke(colors),
        egui::StrokeKind::Inside,
    );

    // Centre the text inside the fixed-width field.
    ui.painter().galley(
        egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        colors.fg_subtle(),
    );
}
