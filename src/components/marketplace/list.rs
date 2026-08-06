// Marketplace list — design `screens.html` §3 “Marketplace”, left pane: the
// `.shrow` header, the `.mk-tools` search + sort row, the `.mk-cats` category
// strip, and the `.mk-plugs` plugin rows.

use eframe::egui;

use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonSize, IconButton, Input, List, ListEvent, ListItem, ListItemPostfix,
    ListItemPrefix, ListStyle, ListTextStyle, Progress, Select, SelectOption, Separator,
    SidebarHeader, Size, Spinner, Typography, TypographyVariant,
};
use thoth_plugin_sdk::theme::{FONT_CONTROL, color_to_hex};

use crate::theme::ThemeColors;

use super::state::{InstallState, MarketplaceUiState, SortOrder, category_glyph, category_label};

// ── Tools block — design `.mk-tools` ─────────────────────────────────────────

/// `.mk-tools{padding:4px 10px 8px}`.
const TOOLS_PAD_X: i8 = 10;
const TOOLS_PAD_TOP: i8 = 4;
const TOOLS_PAD_BOTTOM: i8 = 8;
/// `.mk-tools{gap:7px}` between the search field and the sort row.
const TOOLS_GAP: f32 = 7.0;
/// The sort row's own `style="display:flex;gap:6px"`.
const SORT_GAP: f32 = 6.0;
/// `.selbox{height:26px}` — `Size::Small`'s 24px trigger is the nearest rung on
/// the shared control scale that still sits under the 28px field above it.
const SORT_H: f32 = 24.0;
/// `.ib{width:26px}` — the framed refresh button, i.e. `Size::Medium`.
const REFRESH_W: f32 = 26.0;

// ── Category strip — design `.mk-cats` ───────────────────────────────────────

/// `.mk-cats{padding:0 8px 6px}`.
const CATS_PAD_X: i8 = 8;
const CATS_PAD_BOTTOM: i8 = 6;

// ── Plugin rows — design `.mk-plugs` ─────────────────────────────────────────

/// `.mk-plugs{padding:6px}`.
const PLUGS_PAD: i8 = 6;
/// How much of a plugin's description a row shows before it is elided.
const DESC_MAX_CHARS: usize = 200;

// ── Load / error states ──────────────────────────────────────────────────────

/// Padding around the loading and failure lines.
const STATUS_PAD: i8 = 16;

pub(super) fn render(ui: &mut egui::Ui, state: &mut MarketplaceUiState, colors: &ThemeColors) {
    // ── Header: title + count ──────────────────────────────────────────────
    let total = state.plugins.len();
    let visible_count = count_filtered(state);
    let count_text = format!("{visible_count} of {total}");
    ui.add(
        SidebarHeader::builder()
            .title("PLUGIN STORE")
            .trailing_text(count_text)
            .build(),
    );

    // ── Search bar + sort row — design `.mk-tools` ─────────────────────────
    egui::Frame::NONE
        .inner_margin(egui::Margin {
            left: TOOLS_PAD_X,
            right: TOOLS_PAD_X,
            top: TOOLS_PAD_TOP,
            bottom: TOOLS_PAD_BOTTOM,
        })
        .show(ui, |ui| {
            // Row 1: search input
            let mut search_input = Input::builder()
                .value(state.search_query.clone())
                .placeholder("Search plugins…")
                .icon(egui_phosphor::regular::MAGNIFYING_GLASS)
                .rows(1)
                .build();
            if search_input.show(ui).inner {
                state.search_query = search_input.value.clone();
            }

            ui.add_space(TOOLS_GAP);

            // Row 2: sort select (fills available width) + gap + refresh icon
            ui.horizontal(|ui| {
                let select_w = (ui.available_width() - REFRESH_W - SORT_GAP).max(60.0);

                let sort_val = match state.sort {
                    SortOrder::NameAZ => "name_az",
                    SortOrder::NameZA => "name_za",
                };

                // Constrain to the select width before rendering
                let mut sort_select = Select::builder()
                    .id("mp_sort_select")
                    .value(sort_val.to_string())
                    .options(vec![
                        SelectOption::builder()
                            .value("name_az")
                            .label("Name (A–Z)")
                            .build(),
                        SelectOption::builder()
                            .value("name_za")
                            .label("Name (Z–A)")
                            .build(),
                    ])
                    .prefix_label("Sort: ")
                    .size(Size::Small)
                    .build();
                let new_val = ui
                    .allocate_ui(egui::vec2(select_w, SORT_H), |ui| {
                        sort_select.show(ui).inner.selected
                    })
                    .inner;
                if let Some(new_val) = new_val {
                    state.sort = match new_val.as_str() {
                        "name_za" => SortOrder::NameZA,
                        _ => SortOrder::NameAZ,
                    };
                }

                ui.add_space(SORT_GAP);

                // `.ib.framed` — surface fill plus the shared hairline edge.
                let resp = ui.add(
                    IconButton::builder()
                        .icon(egui_phosphor::regular::ARROWS_CLOCKWISE)
                        .tooltip("Refresh Registry")
                        .frame(true)
                        .size(Size::Medium)
                        .build(),
                );
                if resp.clicked() {
                    state.load_if_needed(ui.ctx(), true);
                }
            });
        });

    // ── Category strip ─────────────────────────────────────────────────────
    let installed_count = state
        .install_states
        .values()
        .filter(|s| matches!(s, InstallState::Installed | InstallState::Disabled))
        .count();
    let updates_count = 0usize;

    struct CatDef {
        id: String,
        glyph: &'static str,
        label: String,
        count: usize,
    }

    let mut cat_defs: Vec<CatDef> = vec![
        CatDef {
            id: "all".to_string(),
            glyph: egui_phosphor::regular::SQUARES_FOUR,
            label: "All".to_string(),
            count: total,
        },
        CatDef {
            id: "installed".to_string(),
            glyph: egui_phosphor::regular::CHECK_SQUARE,
            label: "Installed".to_string(),
            count: installed_count,
        },
        CatDef {
            id: "updates".to_string(),
            glyph: egui_phosphor::regular::ARROW_CIRCLE_UP,
            label: "Updates".to_string(),
            count: updates_count,
        },
    ];

    // Dynamic category entries from plugin categories
    let mut seen_cats: Vec<String> = Vec::new();
    for p in &state.plugins {
        for cat in &p.categories {
            if !seen_cats.contains(cat) {
                seen_cats.push(cat.clone());
            }
        }
    }
    seen_cats.sort();
    for cat in &seen_cats {
        let count = state
            .plugins
            .iter()
            .filter(|p| p.categories.iter().any(|c| c == cat))
            .count();
        if count > 0 {
            cat_defs.push(CatDef {
                id: cat.clone(),
                glyph: category_glyph(cat),
                label: category_label(cat).to_string(),
                count,
            });
        }
    }

    let cat_items: Vec<ListItem> = cat_defs
        .iter()
        .map(|cat| {
            let is_active = state.selected_category == cat.id;
            // `.cat{color:var(--fg-muted)}` / `.cat.on{color:var(--accent)}` —
            // a selected category tints its glyph *and* its label.
            let label_color = if is_active {
                colors.accent
            } else {
                colors.fg_muted
            };
            // `.cat .c{margin-left:auto;font-family:var(--mono);font-size:10px}`
            // — a bare mono count, with no chip chrome around it.
            let count = (cat.count > 0).then(|| ListItemPostfix::Text {
                text: cat.count.to_string(),
                color: None,
                mono: true,
            });
            ListItem::builder()
                .title(cat.label.clone())
                .title_color(color_to_hex(label_color))
                .prefix(ListItemPrefix::Icon {
                    glyph: cat.glyph.to_string(),
                    color: Some(color_to_hex(label_color)),
                })
                .selected(is_active)
                .maybe_postfix(count)
                .build()
        })
        .collect();

    // `.cat` rows are 30px tall with a 12px label — the flush row shape (8px
    // padding around a 12px title) rather than the 22px compact strip. They are
    // parted by nothing but their own hover fill, hence no separators.
    let cat_event = egui::Frame::NONE
        .inner_margin(egui::Margin {
            left: CATS_PAD_X,
            right: CATS_PAD_X,
            top: 0,
            bottom: CATS_PAD_BOTTOM,
        })
        .show(ui, |ui| {
            List::builder()
                .items(cat_items)
                .style(ListStyle::Flush)
                .shrink_to_fit(true)
                .show_separators(false)
                .build()
                .show(ui)
        })
        .inner;
    if let Some(ListEvent::ItemClicked(idx)) = cat_event
        && let Some(cat) = cat_defs.get(idx)
    {
        state.selected_category = cat.id.clone();
    }

    // `.mk-plugs{border-top:1px solid …}`
    ui.add(Separator::plain());

    // ── Plugin list ────────────────────────────────────────────────────────
    if state.loading {
        egui::Frame::NONE
            .inner_margin(egui::Margin::same(STATUS_PAD))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = TOOLS_GAP;
                    ui.add(Spinner::builder().build());
                    Typography::body_muted(ui, "Loading plugin registry…");
                });
            });
        return;
    }

    if let Some(err) = state.load_error.clone() {
        egui::Frame::NONE
            .inner_margin(egui::Margin::same(STATUS_PAD))
            .show(ui, |ui| {
                ui.add(
                    Typography::builder()
                        .text(format!("Failed to load marketplace: {err}"))
                        .variant(TypographyVariant::BodyMuted)
                        .color(color_to_hex(colors.error))
                        .build(),
                );
            });
        return;
    }

    struct RowData {
        id: String,
        name: String,
        desc: Option<String>,
        by_line: String,
        install_state: InstallState,
        is_selected: bool,
        icon_file: Option<std::path::PathBuf>,
    }

    let query = state.search_query.to_lowercase();
    let mut rows: Vec<RowData> = state
        .plugins
        .iter()
        .filter(|p| {
            let is_installed = matches!(
                state.install_states.get(&p.id),
                Some(InstallState::Installed) | Some(InstallState::Disabled)
            );
            let passes_category = match state.selected_category.as_str() {
                "all" => true,
                "installed" => is_installed,
                "updates" => false,
                cat => p.categories.iter().any(|c| c == cat),
            };
            passes_category
                && (query.is_empty()
                    || p.name.to_lowercase().contains(&query)
                    || p.description.to_lowercase().contains(&query)
                    || p.author.to_lowercase().contains(&query))
        })
        .map(|p| {
            // `.pl` stacks three lines: `.nm` name, `.ds` description, `.by`
            // author · version — the row's title, description and meta tiers.
            let desc = (!p.description.is_empty()).then(|| {
                let truncated: String = p.description.chars().take(DESC_MAX_CHARS).collect();
                let suffix = if p.description.chars().count() > DESC_MAX_CHARS {
                    "…"
                } else {
                    ""
                };
                format!("{truncated}{suffix}")
            });

            RowData {
                is_selected: state.selected_id.as_deref() == Some(p.id.as_str()),
                install_state: state.install_states.get(&p.id).cloned().unwrap_or_default(),
                id: p.id.clone(),
                name: p.name.clone(),
                desc,
                by_line: format!("by {} · v{}", p.author, p.version),
                icon_file: p.get_icon_file(ui.ctx().clone()).ok(),
            }
        })
        .collect();

    match state.sort {
        SortOrder::NameAZ => {} // poll_pending already sorts A-Z
        SortOrder::NameZA => rows.sort_by(|a, b| b.name.cmp(&a.name)),
    }

    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| {
            let postfix = match &row.install_state {
                InstallState::NotInstalled => Some(ListItemPostfix::Button(
                    Button::builder()
                        .label("Install")
                        .color(ButtonColor::Primary)
                        .button_size(ButtonSize::Small)
                        .icon(egui_phosphor::regular::DOWNLOAD_SIMPLE)
                        .build(),
                )),
                InstallState::Installed => None,
                InstallState::Disabled => Some(ListItemPostfix::Button(
                    Button::builder()
                        .label("Enable")
                        .color(ButtonColor::Primary)
                        .button_size(ButtonSize::Small)
                        .icon(egui_phosphor::regular::PLAY)
                        .build(),
                )),
                InstallState::Failed(_) => Some(ListItemPostfix::Button(
                    Button::builder()
                        .label("Retry")
                        .color(ButtonColor::Danger)
                        .button_size(ButtonSize::Small)
                        .icon(egui_phosphor::regular::ARROW_CLOCKWISE)
                        .build(),
                )),
                InstallState::Installing(pct) => Some(ListItemPostfix::Progress(
                    Progress::builder().value(*pct as f64 / 100.0).build(),
                )),
                InstallState::Update => Some(ListItemPostfix::Button(
                    Button::builder()
                        .label("Update")
                        .color(ButtonColor::Primary)
                        .button_size(ButtonSize::Small)
                        .icon(egui_phosphor::regular::UPLOAD)
                        .build(),
                )),
            };

            // `.pl .tile{background:color-mix(accent 15%,transparent);color:accent}`
            // — the tile is always tinted; selection reads from the row wash.
            let prefix = if let Some(icon_path) = &row.icon_file {
                ListItemPrefix::IconFile {
                    path: icon_path.to_string_lossy().into_owned(),
                }
            } else {
                ListItemPrefix::IconTile {
                    glyph: egui_phosphor::regular::PUZZLE_PIECE.to_string(),
                    color: color_to_hex(colors.accent),
                }
            };

            ListItem::builder()
                .title(row.name.clone())
                .maybe_description(row.desc.clone())
                .meta(row.by_line.clone())
                .prefix(prefix)
                .selected(row.is_selected)
                .maybe_postfix(postfix)
                .build()
        })
        .collect();

    // `.pl` rows: transparent, 8px padding, a `--text 5%` hover wash and an
    // `--accent 12%` selected wash, with nothing but air between them — the
    // flush row shape. Card rows would put each plugin on its own filled panel,
    // which the sheet reserves for sidebar-style lists.
    let list_event = egui::Frame::NONE
        .inner_margin(egui::Margin::same(PLUGS_PAD))
        .show(ui, |ui| {
            List::builder()
                .items(items)
                .style(ListStyle::Flush)
                .show_separators(false)
                .empty_label("No plugins found")
                // `.pl .nm{font-size:12.5px;font-weight:600}` over
                // `.pl .ds{font-size:11px}` over a monospace
                // `.pl .by{font-size:10.5px;color:var(--overlay0)}` — the
                // description and author tiers already match the flush row's
                // defaults, so only the size/weight and the mono family differ.
                .text_style(
                    ListTextStyle::builder()
                        .title_size(FONT_CONTROL)
                        .title_bold(true)
                        .meta_mono(true)
                        .build(),
                )
                .build()
                .show(ui)
        })
        .inner;

    match list_event {
        Some(ListEvent::ItemClicked(idx)) => {
            if let Some(row) = rows.get(idx) {
                state.selected_id = Some(row.id.clone());
            }
        }
        Some(ListEvent::PostfixClicked(item_idx)) => {
            if let Some(row) = rows.get(item_idx) {
                state.selected_id = Some(row.id.clone());
                match &row.install_state {
                    InstallState::NotInstalled | InstallState::Update => {
                        if let Some(plugin) = state.plugins.iter().find(|p| p.id == row.id) {
                            let slot = plugin.download_and_install(ui.ctx().clone());
                            state.install_handles.insert(row.id.clone(), slot);
                            state
                                .install_states
                                .insert(row.id.clone(), InstallState::Installing(0));
                        }
                    }
                    InstallState::Disabled => {
                        state
                            .install_states
                            .insert(row.id.clone(), InstallState::Installed);
                    }
                    InstallState::Failed(_) | InstallState::Installing(_) => {
                        state.install_handles.remove(&row.id);
                        state.install_states.remove(&row.id);
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

pub(super) fn count_filtered(state: &MarketplaceUiState) -> usize {
    let query = state.search_query.to_lowercase();
    state
        .plugins
        .iter()
        .filter(|p| {
            let is_installed = matches!(
                state.install_states.get(&p.id),
                Some(InstallState::Installed) | Some(InstallState::Disabled)
            );
            let passes = match state.selected_category.as_str() {
                "all" => true,
                "installed" => is_installed,
                "updates" => false,
                cat => p.categories.iter().any(|c| c == cat),
            };
            passes
                && (query.is_empty()
                    || p.name.to_lowercase().contains(&query)
                    || p.description.to_lowercase().contains(&query)
                    || p.author.to_lowercase().contains(&query))
        })
        .count()
}
