use eframe::egui;

use crate::components::common::button::{ButtonColor, ButtonProps, ButtonSize};
use crate::components::common::input::{Input, InputProps};
use crate::components::common::list::{List, ListItem, ListItemPostfix, ListItemPrefix, ListProps};
use crate::components::common::select::{Select, SelectOption, SelectProps, SelectSize};
use crate::components::common::separator::Separator;
use crate::components::icon_button::{IconButton, IconButtonProps};
use crate::components::traits::StatelessComponent;
use crate::components::typography::Typography;
use crate::theme::ThemeColors;

use super::state::{InstallState, MarketplaceUiState, SortOrder, category_glyph, category_label};

pub(super) fn render(ui: &mut egui::Ui, state: &mut MarketplaceUiState, colors: &ThemeColors) {
    // ── Header: title + count ──────────────────────────────────────────────
    let total = state.plugins.len();
    let visible_count = count_filtered(state);
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(0, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                Typography::bold(ui, "Plugin Store");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{visible_count} of {total}"))
                            .color(colors.fg_muted)
                            .size(10.0),
                    );
                });
            });
        });

    Separator::with_margins(ui, (0.0, 10.0));

    // ── Search bar + sort row ──────────────────────────────────────────────
    egui::Frame::NONE
        .inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 0,
            bottom: 0,
        })
        .show(ui, |ui| {
            // Row 1: search input
            Input::render(
                ui,
                InputProps {
                    value: &mut state.search_query,
                    placeholder: "Search plugins…",
                    icon: Some(egui_phosphor::regular::MAGNIFYING_GLASS),
                    password: false,
                    disabled: false,
                    multiline: false,
                    rows: 1,
                    desired_width: None,
                    id_salt: None,
                },
            );

            ui.add_space(8.0);

            // Row 2: sort select (fills available width) + gap + refresh icon
            ui.horizontal(|ui| {
                let icon_btn_w = 26.0;
                let gap = 6.0;
                let select_w = (ui.available_width() - icon_btn_w - gap).max(60.0);

                let sort_opts = [
                    SelectOption {
                        value: "name_az".into(),
                        label: "Name (A–Z)".into(),
                    },
                    SelectOption {
                        value: "name_za".into(),
                        label: "Name (Z–A)".into(),
                    },
                ];
                let sort_val = match state.sort {
                    SortOrder::NameAZ => "name_az",
                    SortOrder::NameZA => "name_za",
                };

                // Constrain to the select width before rendering
                let select_resp = ui.allocate_ui(egui::vec2(select_w, 22.0), |ui| {
                    Select::render(
                        ui,
                        SelectProps {
                            id_salt: "mp_sort_select",
                            value: sort_val,
                            options: &sort_opts,
                            prefix_label: Some("Sort: "),
                            size: SelectSize::Small,
                        },
                    )
                });
                if let Some(new_val) = select_resp.inner.changed {
                    state.sort = match new_val.as_str() {
                        "name_za" => SortOrder::NameZA,
                        _ => SortOrder::NameAZ,
                    };
                }

                ui.add_space(gap);

                let resp = IconButton::render(
                    ui,
                    IconButtonProps {
                        icon: egui_phosphor::regular::ARROWS_CLOCKWISE,
                        tooltip: Some("Refresh Registry"),
                        ..Default::default()
                    },
                );
                if resp.clicked {
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

    let cat_count_strs: Vec<String> = cat_defs.iter().map(|c| c.count.to_string()).collect();
    let cat_items: Vec<ListItem<'_>> = cat_defs
        .iter()
        .enumerate()
        .map(|(i, cat)| {
            let is_active = state.selected_category == cat.id;
            let icon_color = if is_active {
                colors.accent
            } else {
                colors.fg_muted
            };
            ListItem {
                title: &cat.label,
                description: None,
                prefix: Some(ListItemPrefix::Icon {
                    glyph: cat.glyph,
                    color: Some(icon_color),
                }),
                badge: None,
                postfix: if cat.count > 0 {
                    Some(ListItemPostfix::Badge {
                        text: &cat_count_strs[i],
                        bg: colors.bg_sunken,
                        fg: colors.fg_muted,
                    })
                } else {
                    None
                },
                selected: is_active,
                accent: None,
                tags: &[],
            }
        })
        .collect();

    let cat_out = List::render(
        ui,
        ListProps {
            items: &cat_items,
            empty_label: None,
            shrink_to_fit: true,
            show_separators: false,
            compact: true,
            max_height: None,
        },
    );

    if let Some(idx) = cat_out.row_clicked
        && let Some(cat) = cat_defs.get(idx)
    {
        state.selected_category = cat.id.clone();
    }

    ui.separator();

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

    let items: Vec<ListItem<'_>> = rows
        .iter()
        .map(|row| {
            let postfix = match &row.install_state {
                InstallState::NotInstalled => Some(ListItemPostfix::ActionButton(ButtonProps {
                    label: "Install".to_string(),
                    color: ButtonColor::Primary,
                    button_size: ButtonSize::Small,
                    icon: Some(egui_phosphor::regular::DOWNLOAD_SIMPLE.to_string()),
                    ..Default::default()
                })),
                InstallState::Installed => None,
                InstallState::Disabled => Some(ListItemPostfix::ActionButton(ButtonProps {
                    label: "Enable".to_string(),
                    color: ButtonColor::Primary,
                    button_size: ButtonSize::Small,
                    icon: Some(egui_phosphor::regular::PLAY.to_string()),
                    ..Default::default()
                })),
                InstallState::Failed(_) => Some(ListItemPostfix::ActionButton(ButtonProps {
                    label: "Retry".to_string(),
                    color: ButtonColor::Danger,
                    button_size: ButtonSize::Small,
                    icon: Some(egui_phosphor::regular::ARROW_CLOCKWISE.to_string()),
                    ..Default::default()
                })),
                InstallState::Installing(pct) => Some(ListItemPostfix::ProgressBar(*pct)),
                InstallState::Update => Some(ListItemPostfix::ActionButton(ButtonProps {
                    label: "Update".to_string(),
                    color: ButtonColor::Secondary,
                    button_size: ButtonSize::Small,
                    icon: Some(egui_phosphor::regular::UPLOAD.to_string()),
                    ..Default::default()
                })),
            };

            let icon_color = if row.is_selected {
                colors.accent
            } else {
                colors.fg_muted
            };

            let prefix = if let Some(icon_path) = &row.icon_file {
                ListItemPrefix::IconFile {
                    path: icon_path.to_path_buf(),
                }
            } else {
                ListItemPrefix::IconTile {
                    glyph: egui_phosphor::regular::PUZZLE_PIECE,
                    color: icon_color,
                }
            };

            ListItem {
                title: &row.name,
                description: Some(&row.desc),
                prefix: Some(prefix),
                badge: None,
                postfix,
                selected: row.is_selected,
                accent: None,
                tags: &row.tag_labels,
            }
        })
        .collect();

    let list_output = List::render(
        ui,
        ListProps {
            items: &items,
            empty_label: Some("No plugins found"),
            shrink_to_fit: false,
            show_separators: true,
            compact: false,
            max_height: None,
        },
    );

    if let Some(idx) = list_output.row_clicked
        && let Some(row) = rows.get(idx)
    {
        state.selected_id = Some(row.id.clone());
    }

    if let Some(item_idx) = list_output.postfix_clicked
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
