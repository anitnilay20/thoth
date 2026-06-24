use std::sync::{Arc, Mutex};

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use thoth_plugin_sdk::components::{Button, ButtonColor, ButtonSize, Typography};

use crate::components::common::helpers::load_icon_texture;
use crate::plugin::marketplace::MarketPlacePlugin;
use crate::theme::{ThemeColors, icon_rich_text, phosphor_font_id};

use super::state::{DetailAction, InstallState, ReadmeCacheEntry, category_glyph, category_label};

// ── Entry points ──────────────────────────────────────────────────────────────

pub(super) fn render(
    ui: &mut egui::Ui,
    plugin: &MarketPlacePlugin,
    install_state: &InstallState,
    colors: &ThemeColors,
) -> Option<DetailAction> {
    let mut action = None;

    egui::Frame::NONE.fill(colors.bg).show(ui, |ui| {
        // Header: fixed above scroll, contains icon + content + actions top-right
        render_header(ui, plugin, install_state, colors, &mut action);

        // 1px border-bottom beneath header
        let (line_rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter().rect_filled(line_rect, 0.0, colors.surface);

        // Banners (between header and scroll content)
        if let InstallState::Failed(msg) = install_state {
            render_banner_error(ui, msg, colors, &mut action);
        }
        if let InstallState::Installing(progress) = install_state {
            render_banner_installing(ui, plugin, *progress, colors);
        }

        // Two-column scroll area
        egui::ScrollArea::both()
            .id_salt("mp_detail_scroll")
            .show(ui, |ui| {
                let avail = ui.available_width();
                let sidebar_w = 220.0_f32;
                let gap = 28.0_f32;
                let h_pad = 28.0_f32;
                let readme_w = (avail - sidebar_w - gap - h_pad * 2.0).clamp(100.0, 760.0);

                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left: 28,
                        right: 28,
                        top: 20,
                        bottom: 20,
                    })
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(readme_w);
                                render_readme(ui, plugin, colors);
                            });
                            ui.add_space(gap);
                            ui.vertical(|ui| {
                                ui.set_width(sidebar_w);
                                render_sidebar_meta(ui, plugin, colors);
                            });
                        });
                    });
            });
    });

    action
}

pub(super) fn render_empty(ui: &mut egui::Ui, colors: &ThemeColors) {
    egui::Frame::NONE
        .fill(colors.bg)
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.label(
                        egui::RichText::new(egui_phosphor::regular::PUZZLE_PIECE)
                            .font(phosphor_font_id(48.0))
                            .color(colors.surface_active),
                    );
                    ui.add_space(16.0);
                    Typography::heading(ui, "Plugin Store");
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(
                            "Select a plugin from the list to see its details and install options.\nPlugins extend Thoth with themes, formatters, validators, and integrations.",
                        )
                        .size(13.0)
                        .color(colors.fg_muted),
                    );
                });
            });
        });
}

// ── Header ────────────────────────────────────────────────────────────────────

fn render_header(
    ui: &mut egui::Ui,
    plugin: &MarketPlacePlugin,
    install_state: &InstallState,
    colors: &ThemeColors,
    action: &mut Option<DetailAction>,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin {
            left: 28,
            right: 28,
            top: 20,
            bottom: 16,
        })
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                // 64×64 icon — real image if downloaded, otherwise glyph tile
                let (ir, _) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
                if ui.is_rect_visible(ir) {
                    let icon_path = plugin.get_icon_file(ui.ctx().clone()).ok();
                    let texture = icon_path
                        .as_deref()
                        .and_then(|p| load_icon_texture(ui.ctx(), p, "mp_detail_icon"));

                    if let Some(tex) = texture {
                        ui.put(
                            ir,
                            egui::Image::new(&tex)
                                .fit_to_exact_size(ir.size())
                                .corner_radius(egui::CornerRadius::same(8)),
                        );
                    } else {
                        // Fallback: accent-tinted rounded tile with category glyph
                        ui.painter().rect_filled(
                            ir,
                            6.0,
                            egui::Color32::from_rgba_unmultiplied(
                                colors.accent.r(),
                                colors.accent.g(),
                                colors.accent.b(),
                                0x33,
                            ),
                        );
                        ui.painter().rect_stroke(
                            ir,
                            6.0,
                            egui::Stroke::new(
                                1.0,
                                egui::Color32::from_rgba_unmultiplied(
                                    colors.accent.r(),
                                    colors.accent.g(),
                                    colors.accent.b(),
                                    0x55,
                                ),
                            ),
                            egui::StrokeKind::Middle,
                        );
                        let cap_glyph = plugin
                            .categories
                            .first()
                            .map(|c| category_glyph(c))
                            .unwrap_or(egui_phosphor::regular::PUZZLE_PIECE);
                        ui.painter().text(
                            ir.center(),
                            egui::Align2::CENTER_CENTER,
                            cap_glyph,
                            phosphor_font_id(36.0),
                            colors.accent,
                        );
                    }
                }

                ui.add_space(16.0);

                // Reserve enough for the widest action combo (Disable + Uninstall ≈ 155px).
                let actions_w = 160.0_f32;
                let content_w = (ui.available_width() - actions_w).max(80.0);

                // Content column (explicit width so available_width() is correct inside)
                ui.allocate_ui_with_layout(
                    egui::vec2(content_w, 200.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        // Row 1: name · version · state badge
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            ui.add(
                                Typography::builder()
                                    .text(&plugin.name)
                                    .bold(true)
                                    .size(22.0)
                                    .build(),
                            );
                            ui.label(
                                egui::RichText::new(format!("v{}", plugin.version))
                                    .size(13.0)
                                    .monospace()
                                    .color(colors.fg_muted),
                            );
                            state_badge(ui, install_state, colors);
                        });

                        ui.add_space(4.0);

                        // Description
                        ui.label(
                            egui::RichText::new(&plugin.description)
                                .size(13.0)
                                .color(colors.fg_muted),
                        );

                        ui.add_space(10.0);

                        // Metadata row: author  ·  categories
                        // Each item is an icon+text group; 16px between groups.
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(16.0, 4.0);

                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                ui.label(
                                    icon_rich_text(egui_phosphor::regular::USER, 11.0)
                                        .color(colors.fg_muted),
                                );
                                ui.label(
                                    egui::RichText::new(&plugin.author)
                                        .size(11.0)
                                        .color(colors.fg_muted),
                                );
                            });

                            if !plugin.categories.is_empty() {
                                let cats = plugin
                                    .categories
                                    .iter()
                                    .map(|c| category_label(c))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    ui.label(
                                        icon_rich_text(egui_phosphor::regular::FOLDER, 11.0)
                                            .color(colors.fg_muted),
                                    );
                                    ui.label(
                                        egui::RichText::new(cats).size(11.0).color(colors.fg_muted),
                                    );
                                });
                            }
                        });
                    },
                );

                // Actions — right_to_left fills the remaining space and anchors
                // the buttons to the right edge (no vertical wrapper, which would
                // expand to full width and push buttons back to the left).
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    render_action_buttons(ui, install_state, colors, action);
                });
            });
        });
}

fn state_badge(ui: &mut egui::Ui, install_state: &InstallState, colors: &ThemeColors) {
    let (text, icon, color): (&str, &str, egui::Color32) = match install_state {
        InstallState::Installed => (
            "Installed",
            egui_phosphor::regular::CHECK_CIRCLE,
            colors.success,
        ),
        InstallState::Disabled => (
            "Disabled",
            egui_phosphor::regular::PAUSE_CIRCLE,
            colors.fg_muted,
        ),
        InstallState::Installing(_) => (
            "Installing",
            egui_phosphor::regular::ARROW_CLOCKWISE,
            colors.info,
        ),
        InstallState::Failed(_) => ("Failed", egui_phosphor::regular::WARNING, colors.error),
        InstallState::NotInstalled => return,
        InstallState::Update => (
            "Update",
            egui_phosphor::regular::UPLOAD,
            colors.accent_secondary,
        ),
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.label(icon_rich_text(icon, 11.0).color(color));
        ui.label(egui::RichText::new(text).size(11.0).color(color));
    });
}

// ── Action buttons (inside header, top-right) ─────────────────────────────────

fn render_action_buttons(
    ui: &mut egui::Ui,
    install_state: &InstallState,
    colors: &ThemeColors,
    action: &mut Option<DetailAction>,
) {
    let _ = colors;
    match install_state {
        InstallState::NotInstalled => {
            if ui
                .add(
                    Button::builder()
                        .label("Install")
                        .color(ButtonColor::Primary)
                        .icon(egui_phosphor::regular::DOWNLOAD_SIMPLE)
                        .button_size(ButtonSize::Small)
                        .build(),
                )
                .clicked()
            {
                *action = Some(DetailAction::Install);
            }
        }

        InstallState::Installed => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if ui
                    .add(
                        Button::builder()
                            .label("Disable")
                            .color(ButtonColor::Default)
                            .button_size(ButtonSize::Small)
                            .build(),
                    )
                    .clicked()
                {
                    *action = Some(DetailAction::Disable);
                }
                if ui
                    .add(
                        Button::builder()
                            .label("Uninstall")
                            .color(ButtonColor::Default)
                            .icon(egui_phosphor::regular::TRASH)
                            .button_size(ButtonSize::Small)
                            .build(),
                    )
                    .clicked()
                {
                    *action = Some(DetailAction::Uninstall);
                }
            });
        }

        InstallState::Disabled => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if ui
                    .add(
                        Button::builder()
                            .label("Enable")
                            .color(ButtonColor::Primary)
                            .icon(egui_phosphor::regular::PLAY)
                            .button_size(ButtonSize::Small)
                            .build(),
                    )
                    .clicked()
                {
                    *action = Some(DetailAction::Enable);
                }
                if ui
                    .add(
                        Button::builder()
                            .label("Uninstall")
                            .color(ButtonColor::Default)
                            .icon(egui_phosphor::regular::TRASH)
                            .button_size(ButtonSize::Small)
                            .build(),
                    )
                    .clicked()
                {
                    *action = Some(DetailAction::Uninstall);
                }
            });
        }

        InstallState::Installing(_) => {
            if ui
                .add(
                    Button::builder()
                        .label("Cancel")
                        .color(ButtonColor::Default)
                        .icon(egui_phosphor::regular::X)
                        .button_size(ButtonSize::Small)
                        .build(),
                )
                .clicked()
            {
                *action = Some(DetailAction::Retry); // reuse as cancel signal
            }
        }

        InstallState::Failed(_) => {
            if ui
                .add(
                    Button::builder()
                        .label("Retry install")
                        .color(ButtonColor::Danger)
                        .icon(egui_phosphor::regular::ARROW_CLOCKWISE)
                        .button_size(ButtonSize::Small)
                        .build(),
                )
                .clicked()
            {
                *action = Some(DetailAction::Retry);
            }
        }
        InstallState::Update => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if ui
                    .add(
                        Button::builder()
                            .label("Update")
                            .color(ButtonColor::Secondary)
                            .icon(egui_phosphor::regular::UPLOAD)
                            .button_size(ButtonSize::Small)
                            .build(),
                    )
                    .clicked()
                {
                    *action = Some(DetailAction::Install);
                }
                if ui
                    .add(
                        Button::builder()
                            .label("Disable")
                            .color(ButtonColor::Default)
                            .button_size(ButtonSize::Small)
                            .build(),
                    )
                    .clicked()
                {
                    *action = Some(DetailAction::Disable);
                }
                if ui
                    .add(
                        Button::builder()
                            .label("Uninstall")
                            .color(ButtonColor::Default)
                            .icon(egui_phosphor::regular::TRASH)
                            .button_size(ButtonSize::Small)
                            .build(),
                    )
                    .clicked()
                {
                    *action = Some(DetailAction::Uninstall);
                }
            });
        }
    }
}

// ── Banners ───────────────────────────────────────────────────────────────────

fn render_banner_error(
    ui: &mut egui::Ui,
    msg: &str,
    colors: &ThemeColors,
    action: &mut Option<DetailAction>,
) {
    let err = colors.error;
    let bg = egui::Color32::from_rgba_unmultiplied(err.r(), err.g(), err.b(), 0x15);
    let border = egui::Color32::from_rgba_unmultiplied(err.r(), err.g(), err.b(), 0x55);

    egui::Frame::NONE
        .fill(bg)
        .inner_margin(egui::Margin {
            left: 28,
            right: 28,
            top: 10,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.label(icon_rich_text(egui_phosphor::regular::WARNING, 16.0).color(err));
                ui.vertical(|ui| {
                    ui.add(
                        Typography::builder()
                            .text("Install failed")
                            .bold(true)
                            .color(thoth_plugin_sdk::theme::color_to_hex(err))
                            .build(),
                    );
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new(msg).size(12.0).color(colors.fg_muted));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            Button::builder()
                                .label("Retry")
                                .color(ButtonColor::Danger)
                                .icon(egui_phosphor::regular::ARROW_CLOCKWISE)
                                .button_size(ButtonSize::Small)
                                .build(),
                        )
                        .clicked()
                    {
                        *action = Some(DetailAction::Retry);
                    }
                });
            });
        });
    // Bottom border
    let (line, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().rect_filled(line, 0.0, border);
}

fn render_banner_installing(
    ui: &mut egui::Ui,
    plugin: &MarketPlacePlugin,
    progress: u8,
    colors: &ThemeColors,
) {
    let inf = colors.info;
    let bg = egui::Color32::from_rgba_unmultiplied(inf.r(), inf.g(), inf.b(), 0x10);
    let border = egui::Color32::from_rgba_unmultiplied(inf.r(), inf.g(), inf.b(), 0x55);

    egui::Frame::NONE
        .fill(bg)
        .inner_margin(egui::Margin {
            left: 28,
            right: 28,
            top: 10,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.label(icon_rich_text(egui_phosphor::regular::ARROW_CLOCKWISE, 14.0).color(inf));
                Typography::bold(ui, &format!("Installing {}…", plugin.name));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{}%", progress))
                            .size(11.0)
                            .monospace()
                            .color(colors.fg_muted),
                    );
                });
            });
            ui.add_space(6.0);
            let bar_w = ui.available_width();
            let (track, _) = ui.allocate_exact_size(egui::vec2(bar_w, 4.0), egui::Sense::hover());
            ui.painter().rect_filled(track, 2.0, colors.surface);
            let fill = egui::Rect::from_min_size(
                track.min,
                egui::vec2(track.width() * (progress as f32 / 100.0), 4.0),
            );
            ui.painter().rect_filled(fill, 2.0, inf);
        });
    // Bottom border
    let (line, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().rect_filled(line, 0.0, border);
}

// ── README (left column) ──────────────────────────────────────────────────────

fn render_readme(ui: &mut egui::Ui, plugin: &MarketPlacePlugin, colors: &ThemeColors) {
    let mut entry = ReadmeCacheEntry::load(ui.ctx(), plugin);
    if entry.needs_fetch() {
        entry.start_fetch(ui.ctx(), plugin);
    }
    entry.poll();
    entry.save(ui.ctx(), &plugin.id);

    match (&entry.content, &entry.error, entry.pending.is_some()) {
        (Some(text), _, _) => {
            let cache_id = egui::Id::new("mp_readme_md_cache");
            let cache_arc = ui.ctx().data_mut(|d| {
                d.get_temp::<Arc<Mutex<CommonMarkCache>>>(cache_id)
                    .unwrap_or_else(|| Arc::new(Mutex::new(CommonMarkCache::default())))
            });
            {
                let mut cache = cache_arc.lock().unwrap();
                egui::Frame::NONE.fill(colors.bg).show(ui, |ui| {
                    ui.set_height(ui.available_height());
                    CommonMarkViewer::new().show(ui, &mut cache, text);
                });
            }
            ui.ctx().data_mut(|d| d.insert_temp(cache_id, cache_arc));
        }
        (_, Some(err), _) => {
            ui.label(
                egui::RichText::new(format!("Failed to load README: {err}"))
                    .size(12.0)
                    .color(colors.fg_muted),
            );
        }
        _ => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    egui::RichText::new("Loading README…")
                        .size(12.0)
                        .color(colors.fg_muted),
                );
            });
        }
    }
}

// ── Sidebar metadata (right column, 220 px) ───────────────────────────────────

fn render_sidebar_meta(ui: &mut egui::Ui, plugin: &MarketPlacePlugin, colors: &ThemeColors) {
    // Identifier
    meta_label(ui, "Identifier");
    ui.label(
        egui::RichText::new(&plugin.id)
            .size(12.0)
            .monospace()
            .color(colors.fg),
    );
    ui.add_space(14.0);

    // Version
    meta_label(ui, "Version");
    ui.label(
        egui::RichText::new(&plugin.version)
            .size(12.0)
            .monospace()
            .color(colors.fg),
    );
    ui.add_space(14.0);

    // Author
    meta_label(ui, "Author");
    ui.label(
        egui::RichText::new(&plugin.author)
            .size(12.0)
            .color(colors.fg),
    );
    ui.add_space(14.0);

    // Repository
    if !plugin.repo_url.is_empty() {
        meta_label(ui, "Repository");
        if ui
            .link(
                egui::RichText::new(plugin.repo_url.trim_start_matches("https://"))
                    .size(12.0)
                    .color(colors.accent),
            )
            .clicked()
        {
            ui.ctx()
                .open_url(egui::OpenUrl::new_tab(plugin.repo_url.clone()));
        };
        ui.add_space(14.0);
    }

    // SHA-256 (truncated, shown in code block style)
    if !plugin.sha256.is_empty() {
        meta_label(ui, "SHA-256");
        let sha_short = if plugin.sha256.len() > 32 {
            format!("{}…", &plugin.sha256[..32])
        } else {
            plugin.sha256.clone()
        };
        egui::Frame::NONE
            .fill(colors.bg_sunken)
            .corner_radius(4)
            .stroke(egui::Stroke::new(1.0, colors.surface))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(&sha_short)
                        .size(10.0)
                        .monospace()
                        .color(colors.fg_muted),
                );
            });
    }
}

/// Renders the uppercase bold label used above each sidebar metadata value.
/// 10 px · bold · uppercase · `sidebar_header` color (overlay2 equivalent).
fn meta_label(ui: &mut egui::Ui, text: &str) {
    Typography::panel_header(ui, &text.to_uppercase());
    ui.add_space(4.0);
}
