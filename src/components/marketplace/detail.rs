// Marketplace detail — design `screens.html` §3 “Marketplace”, right pane: the
// `.dt-head` identity block with its `.dt-actions`, over a `.dt-body` two-column
// grid of README and `.meta-side` metadata.

use std::sync::{Arc, Mutex};

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use thoth_plugin_sdk::components::{
    Badge, Button, ButtonColor, ButtonSize, ButtonType, Icon, Progress, Separator, Size, Spinner,
    Typography, TypographyVariant,
};

use crate::components::common::helpers::load_icon_texture;
use crate::plugin::marketplace::MarketPlacePlugin;
use crate::theme::{
    FONT_CAPTION, FONT_CONTROL, RADIUS_CHIP, RADIUS_WINDOW, ThemeColors, color_to_hex, edge_stroke,
    phosphor_font_id, with_alpha,
};

use super::state::{DetailAction, InstallState, ReadmeCacheEntry, category_glyph, category_label};

// ── Head — design `.dt-head` ─────────────────────────────────────────────────

/// `.dt-head{padding:20px 26px 16px}`.
const HEAD_PAD_X: i8 = 26;
const HEAD_PAD_TOP: i8 = 20;
const HEAD_PAD_BOTTOM: i8 = 16;
/// `.dt-head{gap:16px}`.
const HEAD_GAP: f32 = 16.0;
/// `.dt-icon{width:64px;height:64px}`.
const ICON_BOX: f32 = 64.0;
/// `.dt-icon{font-size:34px}`.
const ICON_GLYPH: f32 = 34.0;
/// `.dt-icon{background:color-mix(accent 16%,transparent)}`.
const ICON_TINT_ALPHA: u8 = 41; // 16% of 255
/// `.dt-nm{font-size:22px;font-weight:700}`.
const NAME_FONT: f32 = 22.0;
/// `.dt-t{gap:10px}`.
const TITLE_GAP: f32 = 10.0;
/// `.dt-v{font-family:var(--mono);font-size:13px}`.
const VERSION_FONT: f32 = 13.0;
/// `.dt-desc{margin-top:7px}`.
const DESC_TOP: f32 = 7.0;
/// `.dt-meta-inline{margin-top:9px}`.
const META_TOP: f32 = 9.0;
/// `.dt-meta-inline{gap:16px}` between the author and category groups.
const META_GAP: f32 = 16.0;
/// `.dt-meta-inline i{margin-right:5px}`.
const META_ICON_GAP: f32 = 5.0;
/// `.dt-actions{gap:8px}`.
const ACTIONS_GAP: f32 = 8.0;

// ── Body — design `.dt-body` ─────────────────────────────────────────────────

/// `.dt-body{padding:20px 26px}`.
const BODY_PAD_X: i8 = 26;
const BODY_PAD_Y: i8 = 20;
/// `.dt-body{grid-template-columns:1fr 220px}`.
const SIDEBAR_W: f32 = 220.0;
/// `.dt-body{gap:28px}`.
const BODY_GAP: f32 = 28.0;
/// The README column never grows past a comfortable measure.
const README_MAX_W: f32 = 760.0;

// ── Metadata side — design `.meta-side` ──────────────────────────────────────

/// `.meta-side .mf{margin-bottom:13px}`.
const META_FIELD_GAP: f32 = 13.0;
/// `.meta-side .ml{font-size:10px;font-weight:700;text-transform:uppercase}`.
const META_LABEL_FONT: f32 = 10.0;
/// `.meta-side .ml{margin-bottom:3px}`.
const META_LABEL_GAP: f32 = 3.0;
/// `.meta-side .mv.mono{font-size:11.5px}` — the caption rung of the type scale.
const META_MONO_FONT: f32 = FONT_CAPTION;
/// `.meta-side .sha{padding:6px 8px}`.
const SHA_PAD_X: i8 = 8;
const SHA_PAD_Y: i8 = 6;
/// How much of the digest the field shows before eliding it.
const SHA_CHARS: usize = 32;

// ── Banners (no sheet counterpart — Thoth-only install feedback) ─────────────

/// Tint behind a banner — the sheet's 8% status wash.
const BANNER_TINT_ALPHA: u8 = 20; // 8% of 255
/// …and the hairline under it, at the `--edge` 34%.
const BANNER_RULE_ALPHA: u8 = 87; // 34% of 255
/// Banner padding, aligned with `.dt-head`'s side gutter.
const BANNER_PAD_Y: i8 = 10;
/// Gap between a banner's icon and its text.
const BANNER_GAP: f32 = 10.0;

// ── Empty state (no sheet counterpart) ───────────────────────────────────────

/// Placeholder glyph size.
const EMPTY_GLYPH: f32 = 48.0;
/// Space above the placeholder glyph.
const EMPTY_TOP: f32 = 80.0;
/// Space under it, and under the heading.
const EMPTY_GAP: f32 = 16.0;

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

        // `.dt-head{box-shadow:inset 0 -1px 0 var(--surface)}`
        ui.add(Separator::rule(color_to_hex(colors.surface)));

        // Banners (between header and scroll content)
        if let InstallState::Failed(msg) = install_state {
            render_banner_error(ui, msg, colors, &mut action);
        }
        if let InstallState::Installing(progress) = install_state {
            render_banner_installing(ui, plugin, *progress, colors);
        }

        // `.dt-body{display:grid;grid-template-columns:1fr 220px;gap:28px}`
        egui::ScrollArea::both()
            .id_salt("mp_detail_scroll")
            .show(ui, |ui| {
                let avail = ui.available_width();
                let readme_w = (avail - SIDEBAR_W - BODY_GAP - f32::from(BODY_PAD_X) * 2.0)
                    .clamp(100.0, README_MAX_W);

                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left: BODY_PAD_X,
                        right: BODY_PAD_X,
                        top: BODY_PAD_Y,
                        bottom: BODY_PAD_Y,
                    })
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.vertical(|ui| {
                                ui.set_width(readme_w);
                                render_readme(ui, plugin, colors);
                            });
                            ui.add_space(BODY_GAP);
                            ui.vertical(|ui| {
                                ui.set_width(SIDEBAR_W);
                                render_sidebar_meta(ui, plugin, colors);
                            });
                        });
                    });
            });
    });

    action
}

pub(super) fn render_empty(ui: &mut egui::Ui, colors: &ThemeColors) {
    egui::Frame::NONE.fill(colors.bg).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(EMPTY_TOP);
                ui.add(
                    Icon::builder()
                        .glyph(egui_phosphor::regular::PUZZLE_PIECE)
                        .size(EMPTY_GLYPH)
                        .color(color_to_hex(colors.surface_active))
                        .build(),
                );
                ui.add_space(EMPTY_GAP);
                Typography::heading(ui, "Plugin Store");
                ui.add_space(ACTIONS_GAP);
                ui.add(
                    Typography::builder()
                        .text(
                            "Select a plugin from the list to see its details and install options.\nPlugins extend Thoth with themes, formatters, validators, and integrations.",
                        )
                        .variant(TypographyVariant::Subtitle)
                        .build(),
                );
            });
        });
    });
}

// ── Head ──────────────────────────────────────────────────────────────────────

fn render_header(
    ui: &mut egui::Ui,
    plugin: &MarketPlacePlugin,
    install_state: &InstallState,
    colors: &ThemeColors,
    action: &mut Option<DetailAction>,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin {
            left: HEAD_PAD_X,
            right: HEAD_PAD_X,
            top: HEAD_PAD_TOP,
            bottom: HEAD_PAD_BOTTOM,
        })
        .show(ui, |ui| {
            // `.dt-actions{margin-left:auto}` — the buttons are laid out first so
            // they keep their natural width and the identity block takes the rest,
            // exactly as the CSS grid does. `Align::TOP` (not `Center`) so the row
            // stays as tall as its content instead of centring in the pane.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                // Every gap in the row is explicit.
                ui.spacing_mut().item_spacing.x = 0.0;

                // Right-to-left, so the design order is added back-to-front.
                for (i, (button, act)) in
                    action_buttons(install_state).into_iter().rev().enumerate()
                {
                    if i > 0 {
                        ui.add_space(ACTIONS_GAP);
                    }
                    if ui.add(button).clicked() {
                        *action = Some(act);
                    }
                }
                ui.add_space(HEAD_GAP);

                // `.dt-icon` + `.dt-main` fill what is left of the row. The
                // height is only a hint — the block is allocated at whatever its
                // content measures, so a long description still fits.
                let rest = ui.available_width();
                ui.allocate_ui_with_layout(
                    egui::vec2(rest, ICON_BOX),
                    egui::Layout::left_to_right(egui::Align::TOP),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        render_icon(ui, plugin, colors);
                        ui.add_space(HEAD_GAP);
                        ui.vertical(|ui| render_identity(ui, plugin, install_state, colors));
                    },
                );
            });
        });
}

/// `.dt-icon`: the plugin's own 64px icon, or an accent-tinted tile with its
/// category glyph.
fn render_icon(ui: &mut egui::Ui, plugin: &MarketPlacePlugin, colors: &ThemeColors) {
    let (ir, _) = ui.allocate_exact_size(egui::Vec2::splat(ICON_BOX), egui::Sense::hover());
    if !ui.is_rect_visible(ir) {
        return;
    }

    let texture = plugin
        .get_icon_file(ui.ctx().clone())
        .ok()
        .and_then(|p| load_icon_texture(ui.ctx(), &p, "mp_detail_icon"));

    if let Some(tex) = texture {
        ui.put(
            ir,
            egui::Image::new(&tex)
                .fit_to_exact_size(ir.size())
                .corner_radius(egui::CornerRadius::from(RADIUS_WINDOW)),
        );
        return;
    }

    ui.painter().rect_filled(
        ir,
        egui::CornerRadius::from(RADIUS_WINDOW),
        with_alpha(colors.accent, ICON_TINT_ALPHA),
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
        phosphor_font_id(ICON_GLYPH),
        colors.accent,
    );
}

/// `.dt-main`: name · version · state badge, the description, then the inline
/// author / categories metadata.
fn render_identity(
    ui: &mut egui::Ui,
    plugin: &MarketPlacePlugin,
    install_state: &InstallState,
    colors: &ThemeColors,
) {
    // `.dt-t{display:flex;align-items:center;gap:10px;flex-wrap:wrap}`
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = TITLE_GAP;
        ui.add(
            Typography::builder()
                .text(&plugin.name)
                .variant(TypographyVariant::Heading)
                .size(NAME_FONT)
                .bold(true)
                .build(),
        );
        ui.add(
            Typography::builder()
                .text(format!("v{}", plugin.version))
                .variant(TypographyVariant::Mono)
                .color(color_to_hex(colors.fg_muted))
                .size(VERSION_FONT)
                .build(),
        );
        state_badge(ui, install_state, colors);
    });

    // `.dt-desc{font-size:13px;color:var(--fg-muted);margin-top:7px}`
    ui.add_space(DESC_TOP);
    ui.add(
        Typography::builder()
            .text(&plugin.description)
            .variant(TypographyVariant::Subtitle)
            .build(),
    );

    // `.dt-meta-inline`: author and categories, each an icon + caption group.
    ui.add_space(META_TOP);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(META_GAP, META_ICON_GAP);
        meta_inline(ui, egui_phosphor::regular::USER, &plugin.author, colors);
        if !plugin.categories.is_empty() {
            let cats = plugin
                .categories
                .iter()
                .map(|c| category_label(c))
                .collect::<Vec<_>>()
                .join(", ");
            meta_inline(ui, egui_phosphor::regular::FOLDER, &cats, colors);
        }
    });
}

/// One `.dt-meta-inline` group: a caption-sized glyph, 5px, then the value.
fn meta_inline(ui: &mut egui::Ui, glyph: &str, text: &str, colors: &ThemeColors) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add(
            Icon::builder()
                .glyph(glyph)
                .size(FONT_CAPTION)
                .color(color_to_hex(colors.fg_muted))
                .build(),
        );
        ui.add_space(META_ICON_GAP);
        Typography::caption(ui, text);
    });
}

/// `.dt-badge`: a fully-round soft status pill — an 18% wash of the status colour
/// behind a leading glyph and an 11px proportional label
/// (`<i class="ph ph-check-circle"></i>Installed`).
fn state_badge(ui: &mut egui::Ui, install_state: &InstallState, colors: &ThemeColors) {
    let (text, color, glyph): (&str, egui::Color32, &str) = match install_state {
        InstallState::Installed => (
            "Installed",
            colors.success,
            egui_phosphor::regular::CHECK_CIRCLE,
        ),
        InstallState::Disabled => (
            "Disabled",
            colors.fg_muted,
            egui_phosphor::regular::PAUSE_CIRCLE,
        ),
        InstallState::Installing(_) => (
            "Installing",
            colors.info,
            egui_phosphor::regular::ARROW_CIRCLE_DOWN,
        ),
        InstallState::Failed(_) => (
            "Failed",
            colors.error,
            egui_phosphor::regular::WARNING_CIRCLE,
        ),
        InstallState::NotInstalled => return,
        InstallState::Update => (
            "Update",
            colors.accent_secondary,
            egui_phosphor::regular::ARROW_CIRCLE_UP,
        ),
    };
    ui.add(
        Badge::builder()
            .label(text)
            .icon(glyph)
            .color(color_to_hex(color))
            .soft(true)
            .pill(true)
            // The label is prose in the body family, not a code token.
            .mono(false)
            .size(Size::Large)
            .font_size(FONT_CAPTION)
            .build(),
    );
}

// ── Action buttons (inside the head, top-right) ───────────────────────────────

/// The `.dt-actions` buttons for a state, in design (left-to-right) order.
///
/// The semantic hues are deliberate: `Uninstall` is destructive and irreversible
/// so it carries the error hue, but `Soft` keeps it a red-labelled wash rather
/// than a solid red slab shouting over the button beside it; `Disable` is
/// cautionary but reversible, so it takes the warning hue as a soft tint; and
/// `Enable` is the call to action for a disabled plugin, so it stays solid
/// accent.
fn action_buttons(install_state: &InstallState) -> Vec<(Button, DetailAction)> {
    let uninstall = || {
        (
            Button::builder()
                .label("Uninstall")
                .color(ButtonColor::Danger)
                .button_type(ButtonType::Soft)
                .icon(egui_phosphor::regular::TRASH)
                .button_size(ButtonSize::Medium)
                .build(),
            DetailAction::Uninstall,
        )
    };
    let disable = || {
        (
            Button::builder()
                .label("Disable")
                .color(ButtonColor::Warning)
                .button_type(ButtonType::Soft)
                .icon(egui_phosphor::regular::PAUSE)
                .button_size(ButtonSize::Medium)
                .build(),
            DetailAction::Disable,
        )
    };
    let update = || {
        (
            Button::builder()
                .label("Update")
                .color(ButtonColor::Primary)
                .icon(egui_phosphor::regular::UPLOAD)
                .button_size(ButtonSize::Medium)
                .build(),
            DetailAction::Install,
        )
    };

    match install_state {
        InstallState::NotInstalled => vec![(
            Button::builder()
                .label("Install")
                .color(ButtonColor::Primary)
                .icon(egui_phosphor::regular::DOWNLOAD_SIMPLE)
                .button_size(ButtonSize::Medium)
                .build(),
            DetailAction::Install,
        )],
        InstallState::Installed => vec![disable(), uninstall()],
        InstallState::Disabled => vec![
            (
                Button::builder()
                    .label("Enable")
                    .color(ButtonColor::Primary)
                    .icon(egui_phosphor::regular::PLAY)
                    .button_size(ButtonSize::Medium)
                    .build(),
                DetailAction::Enable,
            ),
            uninstall(),
        ],
        // `Retry` doubles as the cancel signal for an in-flight install.
        InstallState::Installing(_) => vec![(
            Button::builder()
                .label("Cancel")
                .color(ButtonColor::Default)
                .icon(egui_phosphor::regular::X)
                .button_size(ButtonSize::Medium)
                .build(),
            DetailAction::Retry,
        )],
        InstallState::Failed(_) => vec![(
            Button::builder()
                .label("Retry install")
                .color(ButtonColor::Danger)
                .icon(egui_phosphor::regular::ARROW_CLOCKWISE)
                .button_size(ButtonSize::Medium)
                .build(),
            DetailAction::Retry,
        )],
        InstallState::Update => vec![update(), disable(), uninstall()],
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

    egui::Frame::NONE
        .fill(with_alpha(err, BANNER_TINT_ALPHA))
        .inner_margin(egui::Margin {
            left: HEAD_PAD_X,
            right: HEAD_PAD_X,
            top: BANNER_PAD_Y,
            bottom: BANNER_PAD_Y,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = BANNER_GAP;
                ui.add(
                    Icon::builder()
                        .glyph(egui_phosphor::regular::WARNING)
                        .size(HEAD_GAP)
                        .color(color_to_hex(err))
                        .build(),
                );
                ui.vertical(|ui| {
                    ui.add(
                        Typography::builder()
                            .text("Install failed")
                            .bold(true)
                            .color(color_to_hex(err))
                            .build(),
                    );
                    Typography::body_muted(ui, msg);
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
    banner_rule(ui, with_alpha(err, BANNER_RULE_ALPHA));
}

fn render_banner_installing(
    ui: &mut egui::Ui,
    plugin: &MarketPlacePlugin,
    progress: u8,
    colors: &ThemeColors,
) {
    let inf = colors.info;

    egui::Frame::NONE
        .fill(with_alpha(inf, BANNER_TINT_ALPHA))
        .inner_margin(egui::Margin {
            left: HEAD_PAD_X,
            right: HEAD_PAD_X,
            top: BANNER_PAD_Y,
            bottom: BANNER_PAD_Y,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = BANNER_GAP;
                ui.add(Spinner::builder().size(FONT_CONTROL).build());
                Typography::bold(ui, &format!("Installing {}…", plugin.name));
            });
            ui.add_space(META_ICON_GAP);
            ui.add(
                Progress::builder()
                    .value(f64::from(progress) / 100.0)
                    .color(color_to_hex(inf))
                    .readout(format!("{progress}%"))
                    .build(),
            );
        });
    banner_rule(ui, with_alpha(inf, BANNER_RULE_ALPHA));
}

/// The hairline closing a banner off from the content under it.
fn banner_rule(ui: &mut egui::Ui, color: egui::Color32) {
    ui.add(Separator::rule(color_to_hex(color)));
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
            // The SDK's `Markdown` builds a fresh `CommonMarkCache` per call,
            // which would re-parse a long README every frame — so the viewer is
            // driven directly off a cache kept in egui memory.
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
            Typography::body_muted(ui, &format!("Failed to load README: {err}"));
        }
        _ => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = BANNER_GAP;
                ui.add(Spinner::builder().build());
                Typography::body_muted(ui, "Loading README…");
            });
        }
    }
}

// ── Metadata side (right column, 220 px) ──────────────────────────────────────

fn render_sidebar_meta(ui: &mut egui::Ui, plugin: &MarketPlacePlugin, colors: &ThemeColors) {
    // `.mv.mono` — identifier and version.
    meta_field(ui, "Identifier", |ui| {
        ui.add(
            Typography::builder()
                .text(&plugin.id)
                .variant(TypographyVariant::Mono)
                .size(META_MONO_FONT)
                .build(),
        );
    });
    meta_field(ui, "Version", |ui| {
        ui.add(
            Typography::builder()
                .text(&plugin.version)
                .variant(TypographyVariant::Mono)
                .size(META_MONO_FONT)
                .build(),
        );
    });

    // `.mv{font-size:12.5px;color:var(--fg)}`
    meta_field(ui, "Author", |ui| {
        ui.add(
            Typography::builder()
                .text(&plugin.author)
                .size(FONT_CONTROL)
                .build(),
        );
    });

    // `.mv.link{color:var(--accent2)}`
    if !plugin.repo_url.is_empty() {
        meta_field(ui, "Repository", |ui| {
            if ui
                .link(
                    egui::RichText::new(plugin.repo_url.trim_start_matches("https://"))
                        .size(FONT_CONTROL)
                        .color(colors.accent_secondary),
                )
                .clicked()
            {
                ui.ctx()
                    .open_url(egui::OpenUrl::new_tab(plugin.repo_url.clone()));
            }
        });
    }

    // `.sha`: mono caption on `--bg-sunken` behind the shared hairline edge.
    if !plugin.sha256.is_empty() {
        let sha_short = if plugin.sha256.len() > SHA_CHARS {
            format!("{}…", &plugin.sha256[..SHA_CHARS])
        } else {
            plugin.sha256.clone()
        };
        meta_field(ui, "SHA-256", |ui| {
            egui::Frame::NONE
                .fill(colors.bg_sunken)
                .corner_radius(RADIUS_CHIP)
                .stroke(edge_stroke(colors))
                .inner_margin(egui::Margin::symmetric(SHA_PAD_X, SHA_PAD_Y))
                .show(ui, |ui| {
                    ui.add(
                        Typography::builder()
                            .text(&sha_short)
                            .variant(TypographyVariant::Mono)
                            .color(color_to_hex(colors.fg_muted))
                            .size(FONT_CAPTION)
                            .build(),
                    );
                });
        });
    }
}

/// One `.mf`: the 10px uppercase `.ml` label, 3px, the value, then 13px of air.
fn meta_field(ui: &mut egui::Ui, label: &str, value: impl FnOnce(&mut egui::Ui)) {
    ui.add(
        Typography::builder()
            .text(label.to_uppercase())
            .variant(TypographyVariant::GroupLabel)
            .size(META_LABEL_FONT)
            .bold(true)
            .build(),
    );
    ui.add_space(META_LABEL_GAP);
    value(ui);
    ui.add_space(META_FIELD_GAP);
}
