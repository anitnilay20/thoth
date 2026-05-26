use eframe::egui;
use egui_phosphor::regular as ph;

use crate::components::common::traits::StatelessComponent;
use crate::components::common::typography::{Typography, TypographyProps, TypographyVariant};
use crate::theme::ThemeColors;

pub enum WelcomeEvent {
    OpenFilePicker,
    OpenRecentFile(std::path::PathBuf),
}

pub struct WelcomePanel;

impl WelcomePanel {
    pub fn render(
        ui: &mut egui::Ui,
        recent_files: &[String],
        colors: Option<ThemeColors>,
    ) -> Vec<WelcomeEvent> {
        let mut events = Vec::new();

        let c = colors.unwrap_or_else(|| {
            ui.ctx()
                .memory(|m| {
                    m.data
                        .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                })
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        // ── PluginShell header (title row + divider) ──────────────────────────
        // padding: 20px 24px 16px  — top 20, sides 24, bottom 16
        let header_padding = egui::Margin {
            left: 24,
            right: 24,
            top: 20,
            bottom: 16,
        };
        egui::Frame::NONE
            .inner_margin(header_padding)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    // Title — --primary (mauve), --fs-2xl (20px), weight 700
                    Typography::render(
                        ui,
                        TypographyProps {
                            text: "Welcome to Thoth",
                            variant: TypographyVariant::Heading,
                            color: Some(c.accent_secondary),
                            size_override: Some(22.0),
                            bold: true,
                            ..Default::default()
                        },
                    );
                    // Subtitle — --overlay1, --fs-md (13px), baseline-aligned
                    Typography::render(
                        ui,
                        TypographyProps {
                            text: "Wisdom for your JSON.",
                            variant: TypographyVariant::Subtitle,
                            ..Default::default()
                        },
                    );
                });
            });

        // Border-bottom: 1px solid --surface0
        let sep_rect =
            egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), 1.0));
        ui.painter().rect_filled(sep_rect, 0.0, c.surface);
        ui.advance_cursor_after_rect(sep_rect);

        // ── Body — padding: 24px, scrollable ─────────────────────────────────
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let body_padding = egui::Margin::same(24);
                egui::Frame::NONE
                    .inner_margin(body_padding)
                    .show(ui, |ui| {
                        // Grid: 1fr 1fr, gap: 32, maxWidth: 880
                        ui.set_max_width(880.0);
                        ui.spacing_mut().item_spacing.y = 0.0;

                        ui.columns(2, |cols| {
                            let col_gap = 16.0; // half of gap: 32
                            cols[0].add_space(0.0);

                            // ── Left: Start + Recent ──────────────────────────
                            {
                                let ui = &mut cols[0];
                                ui.set_max_width(ui.available_width() - col_gap);

                                t_section(ui, "Start", c);
                                ui.add_space(8.0);

                                if action_row(ui, ph::FOLDER_OPEN, "Open file…", Some("⌘O"), c) {
                                    events.push(WelcomeEvent::OpenFilePicker);
                                }
                                if action_row(ui, ph::APP_WINDOW, "New window", Some("⌘N"), c) {
                                    // handled by shortcut; no event needed
                                }
                                if action_row(ui, ph::PUZZLE_PIECE, "Browse plugins…", None, c) {
                                    // future: open settings → plugins
                                }

                                ui.add_space(24.0);
                                t_section(ui, "Recent", c);
                                ui.add_space(8.0);

                                if recent_files.is_empty() {
                                    Typography::render(
                                        ui,
                                        TypographyProps {
                                            text: "No recent files",
                                            variant: TypographyVariant::BodyMuted,
                                            ..Default::default()
                                        },
                                    );
                                } else {
                                    for path_str in recent_files.iter().take(8) {
                                        let path = std::path::PathBuf::from(path_str);
                                        let name = path
                                            .file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or(path_str.as_str())
                                            .to_string();
                                        let ext = path
                                            .extension()
                                            .and_then(|e| e.to_str())
                                            .map(|e| e.to_uppercase());
                                        if action_row(
                                            ui,
                                            ph::FILE_TEXT,
                                            &name,
                                            ext.as_deref(),
                                            c,
                                        ) {
                                            events.push(WelcomeEvent::OpenRecentFile(path));
                                        }
                                    }
                                }
                            }

                            // ── Right: Tips ───────────────────────────────────
                            {
                                let ui = &mut cols[1];
                                ui.add_space(col_gap);

                                t_section(ui, "Tips", c);
                                ui.add_space(8.0);

                                tip_row(ui, "Drag tab → edge", "Drop on the left, right, top, or bottom of any pane to split it.", c);
                                tip_row(ui, "Drag tab → strip", "Move a tab between groups, or reorder within the same strip.", c);
                                tip_row(ui, "Right-click tab", "Pin, close others, split right, split down — full VSCode-style menu.", c);
                                tip_row(ui, "⌘W", "Close active tab. Last welcome tab closes the window.", c);
                                tip_row(ui, "⌘⌥→ / ⌘⌥←", "Cycle to next or previous tab.", c);
                                tip_row(ui, "⌘1 – ⌘9", "Jump to tab by position.", c);
                            }
                        });
                    });
            });

        events
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// `.t-section` — bold 12px content-area section label
fn t_section(ui: &mut egui::Ui, text: &str, _c: ThemeColors) {
    Typography::render(
        ui,
        TypographyProps {
            text,
            variant: TypographyVariant::SectionHeader,
            ..Default::default()
        },
    );
}

/// ActionRow — flex row, icon (16px --accent), label (13px --text, flex:1), optional hint (12px mono --overlay1)
/// Hover: --surface0 bg, borderRadius 4
fn action_row(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    hint: Option<&str>,
    c: ThemeColors,
) -> bool {
    // padding: 8px 10px on each side — total height ~ 13 (font) + 16 (pad) = ~32
    let row_h = 32.0;
    let pad_x = 10.0;
    let available_w = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(available_w, row_h), egui::Sense::click());

    if ui.is_rect_visible(rect) {
        // Hover bg
        if response.hovered() {
            ui.painter().rect_filled(rect, 4.0, c.surface);
        }

        // Icon: left-padded, 16px, --accent
        let icon_x = rect.min.x + pad_x + 8.0; // 10px pad + ~8px to center
        ui.painter().text(
            egui::pos2(icon_x, rect.center().y),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::new(16.0, egui::FontFamily::Name("phosphor".into())),
            c.accent,
        );

        // Hint (right-aligned, monospace 12px --overlay1) — layout first to reserve space
        let hint_w = if let Some(h) = hint {
            let g = ui.painter().layout_no_wrap(
                h.to_string(),
                egui::FontId::monospace(12.0),
                c.fg_muted,
            );
            let gsize = g.size();
            let hint_x = rect.max.x - pad_x - gsize.x;
            ui.painter().galley(
                egui::pos2(hint_x, rect.center().y - gsize.y / 2.0),
                g,
                c.fg_muted,
            );
            gsize.x + pad_x + 4.0
        } else {
            0.0
        };

        // Label (flex:1, 13px --text, single line)
        let label_x = rect.min.x + pad_x + 8.0 + 10.0 + 6.0; // after icon + gap:10
        let label_max_w = (rect.max.x - hint_w - label_x).max(0.0);
        let lg = ui.painter().layout(
            label.to_string(),
            egui::FontId::proportional(13.0),
            c.fg,
            label_max_w,
        );
        ui.painter().galley(
            egui::pos2(label_x, rect.center().y - lg.size().y / 2.0),
            lg,
            c.fg,
        );
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

/// Tip row — badge (mono 12px, --surface0 bg, 2px/8px padding, --text) + body (13px --overlay1)
fn tip_row(ui: &mut egui::Ui, kbd: &str, body: &str, c: ThemeColors) {
    // padding: 8px 0 on the row
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(12.0, 4.0);

        // Badge: padding 2px 8px, borderRadius 4, bg --surface0, color --text
        let badge_font = egui::FontId::monospace(12.0);
        let badge_galley = ui
            .painter()
            .layout_no_wrap(kbd.to_string(), badge_font, c.fg);
        let pad = egui::vec2(8.0, 2.0);
        let badge_size = badge_galley.size() + pad * 2.0;
        let (badge_rect, _) = ui.allocate_exact_size(badge_size, egui::Sense::hover());
        if ui.is_rect_visible(badge_rect) {
            ui.painter().rect_filled(badge_rect, 4.0, c.surface);
            ui.painter()
                .galley(badge_rect.min + pad, badge_galley, c.fg);
        }

        // Body: 13px --overlay1
        Typography::render(
            ui,
            TypographyProps {
                text: body,
                variant: TypographyVariant::BodyMuted,
                color: Some(c.fg_muted),
                size_override: Some(13.0),
                ..Default::default()
            },
        );
    });
    ui.add_space(4.0);
}
