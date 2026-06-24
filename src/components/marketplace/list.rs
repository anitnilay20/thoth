use eframe::egui;

use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonSize, IconButton, Input, List, ListEvent, ListItem, ListItemPostfix,
    ListItemPrefix, Select, SelectOption, SelectSize, Separator, SidebarHeader,
};
use thoth_plugin_sdk::theme::color_to_hex;

use crate::theme::ThemeColors;

use super::state::{InstallState, MarketplaceUiState, SortOrder, category_glyph, category_label};

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

    // ── Search bar + sort row ──────────────────────────────────────────────
    egui::Frame::NONE
        .inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 10,
            bottom: 0,
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

            ui.add_space(8.0);

            // Row 2: sort select (fills available width) + gap + refresh icon
            ui.horizontal(|ui| {
                let icon_btn_w = 26.0;
                let gap = 6.0;
                let select_w = (ui.available_width() - icon_btn_w - gap).max(60.0);

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
                    .size(SelectSize::Small)
                    .build();
                let new_val = ui
                    .allocate_ui(egui::vec2(select_w, 22.0), |ui| sort_select.show(ui).inner)
                    .inner;
                if let Some(new_val) = new_val {
                    state.sort = match new_val.as_str() {
                        "name_za" => SortOrder::NameZA,
                        _ => SortOrder::NameAZ,
                    };
                }

                ui.add_space(gap);

                let resp = ui.add(
                    IconButton::builder()
                        .icon(egui_phosphor::regular::ARROWS_CLOCKWISE)
                        .tooltip("Refresh Registry")
                        .build(),
                );
                if resp.clicked() {
                    state.load_if_needed(ui.ctx(), true);
                }
            });

            ui.add_space(8.0);
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
            let icon_color = if is_active {
                colors.accent
            } else {
                colors.fg_muted
            };
            let badge = (cat.count > 0).then(|| ListItemPostfix::Badge {
                text: cat.count.to_string(),
                bg: Some(color_to_hex(colors.bg_sunken)),
                fg: Some(color_to_hex(colors.fg_muted)),
            });
            ListItem::builder()
                .title(cat.label.clone())
                .prefix(ListItemPrefix::Icon {
                    glyph: cat.glyph.to_string(),
                    color: Some(color_to_hex(icon_color)),
                })
                .selected(is_active)
                .maybe_postfix(badge)
                .build()
        })
        .collect();

    if let Some(ListEvent::ItemClicked(idx)) = List::builder()
        .items(cat_items)
        .shrink_to_fit(true)
        .show_separators(false)
        .compact(true)
        .build()
        .show(ui)
        && let Some(cat) = cat_defs.get(idx)
    {
        state.selected_category = cat.id.clone();
    }

    ui.add(Separator::plain());

    // ── Plugin list ────────────────────────────────────────────────────────
    if state.loading {
        egui::Frame::NONE
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        egui::RichText::new("Loading plugin registry…")
                            .color(colors.fg_muted)
                            .size(12.0),
                    );
                });
            });
        return;
    }

    if let Some(err) = state.load_error.clone() {
        egui::Frame::NONE
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(format!("Failed to load marketplace: {err}"))
                        .color(colors.error)
                        .size(12.0),
                );
            });
        return;
    }

    struct RowData {
        id: String,
        name: String,
        desc: String,
        tag_labels: Vec<&'static str>,
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
            let tag_labels: Vec<&'static str> =
                p.categories.iter().map(|c| category_label(c)).collect();

            let meta = format!("by {} · v{}", p.author, p.version);
            let desc = if p.description.is_empty() {
                meta
            } else {
                let truncated: String = p.description.chars().take(200).collect();
                let suffix = if p.description.chars().count() > 200 {
                    "…"
                } else {
                    ""
                };
                format!("{truncated}{suffix}\n{meta}")
            };

            RowData {
                is_selected: state.selected_id.as_deref() == Some(p.id.as_str()),
                install_state: state.install_states.get(&p.id).cloned().unwrap_or_default(),
                id: p.id.clone(),
                name: p.name.clone(),
                desc,
                tag_labels,
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
                InstallState::Installing(pct) => Some(ListItemPostfix::ProgressBar(*pct)),
                InstallState::Update => Some(ListItemPostfix::Button(
                    Button::builder()
                        .label("Update")
                        .color(ButtonColor::Secondary)
                        .button_size(ButtonSize::Small)
                        .icon(egui_phosphor::regular::UPLOAD)
                        .build(),
                )),
            };

            let icon_color = if row.is_selected {
                colors.accent
            } else {
                colors.fg_muted
            };

            let prefix = if let Some(icon_path) = &row.icon_file {
                ListItemPrefix::IconFile {
                    path: icon_path.to_string_lossy().into_owned(),
                }
            } else {
                ListItemPrefix::IconTile {
                    glyph: egui_phosphor::regular::PUZZLE_PIECE.to_string(),
                    color: color_to_hex(icon_color),
                }
            };

            ListItem::builder()
                .title(row.name.clone())
                .description(row.desc.clone())
                .prefix(prefix)
                .selected(row.is_selected)
                .tags(
                    row.tag_labels
                        .iter()
                        .map(|t| t.to_string())
                        .collect::<Vec<_>>(),
                )
                .maybe_postfix(postfix)
                .build()
        })
        .collect();

    let list_event = List::builder()
        .items(items)
        .empty_label("No plugins found")
        .build()
        .show(ui);

    if let Some(ListEvent::ItemClicked(idx)) = list_event
        && let Some(row) = rows.get(idx)
    {
        state.selected_id = Some(row.id.clone());
    }

    if let Some(ListEvent::PostfixClicked(item_idx)) = list_event
        && let Some(row) = rows.get(item_idx)
    {
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
