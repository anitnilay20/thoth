use std::collections::HashMap;

use crate::app::persistent_state::Bookmark;
use crate::app::tab_manager::TabId;
use crate::components::bookmarks::{Bookmarks, BookmarksEvent, BookmarksProps};
use crate::components::chart_studio::{
    ChartSpec, ChartStudio, ChartStudioEvent, ColumnInfo, ProducerRef,
};
use crate::components::data_source_panel::{
    DataSourcePanel, DataSourcePanelEvent, DataSourcePanelProps,
};
use crate::components::marketplace::{Marketplace, MarketplaceProps};
use crate::components::recent_files::{RecentFiles, RecentFilesEvent, RecentFilesProps};
use crate::components::search::{Search, SearchEvent, SearchProps};
use crate::components::traits::StatelessComponent;
use crate::components::traits::{ContextComponent, StatefulComponent};
use crate::constants::{MAX_SIDEBAR_WIDTH_RATIO, MIN_SIDEBAR_WIDTH};
use crate::plugin::{Plugin, render_node::render_ui_node, wasm_data_source::ConsentRequest};
use crate::search::SearchMessage;
use eframe::egui;
use thoth_plugin_sdk::components::IconButton;

/// Which sidebar section is currently selected
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarSection {
    RecentFiles,
    Search,
    Bookmarks,
    DataSource {
        plugin_id: String,
    },
    /// A ui-component plugin that returned `Some` from `render-sidebar`.
    PluginSidebar {
        plugin_id: String,
    },
    MarketPlace,
    /// The Chart Studio config panel.
    ChartStudio,
}

/// Props passed to the Sidebar (immutable, one-way binding)
pub struct SidebarProps<'a> {
    pub recent_files: &'a [String],
    pub bookmarks: &'a [Bookmark],
    pub current_file_path: Option<&'a str>,
    pub expanded: bool,
    pub sidebar_width: f32,
    pub selected_section: Option<SidebarSection>,
    /// Whether the search section should receive focus (when just opened)
    pub focus_search: bool,
    /// Current search state with results
    pub search_state: &'a crate::search::Search,
    /// Search history for the current file
    pub search_history: Option<&'a Vec<String>>,
    /// All registered data-source plugins — one icon button is shown per plugin.
    pub data_source_plugins: &'a [&'a Plugin],
    /// Pure ui-component plugins (new-ui-component, not data sources) — one icon
    /// button each; clicking opens the plugin in a new tab.
    pub ui_component_plugins: &'a [&'a Plugin],
    /// The plugin_id of the currently active data-source pane (for icon highlight).
    pub active_datasource_plugin_id: Option<&'a str>,
    /// If the active ui-component plugin rendered a sidebar, its (plugin, output) pair.
    pub plugin_sidebar: Option<PluginSidebarInfo<'a>>,
}

pub struct PluginSidebarInfo<'a> {
    pub plugin_id: &'a str,
    pub plugin_name: &'a str,
    pub icon: Option<&'a str>,
    pub output: &'a crate::plugin::render_node::UiOutput,
}

/// Events emitted by the Sidebar
#[derive(Debug, Clone)]
pub enum SidebarEvent {
    OpenFile(String),
    RemoveRecentFile(String),
    OpenFilePicker,
    SectionToggled(SidebarSection),
    /// Open a pure ui-component plugin (by id) in a new tab.
    OpenUiComponentTab(String),
    WidthChanged(f32),
    // Search events
    Search(SearchMessage),
    NavigateToSearchResult {
        record_index: usize,
    },
    ClearSearchHistory,
    // Bookmark events
    NavigateToBookmark {
        file_path: String,
        path: String,
    },
    RemoveBookmark(usize),
    JumpToPath(String),

    // Datasource Plugin Events
    DataSourceQueryResult {
        json: String,
        display_url: String,
    },
    DataSourceConsentNeeded(ConsentRequest),
    DataSourceError(String),
    DataSourceLoading(bool),
    /// A widget interaction from the plugin's sidebar panel.
    PluginSidebarEvent(crate::plugin::render_node::UiEvent),
    OpenSettings,
    // Chart Studio events
    /// The user picked a chart data source; resolve its columns.
    ChartSelectSource(TabId),
    /// Build a chart tab from this spec.
    ChartGenerate(ChartSpec),
    /// Activate an already-open chart tab.
    ChartFocus(TabId),
}

pub struct SidebarOutput {
    pub events: Vec<SidebarEvent>,
}

/// Stateful sidebar component
///
/// This component follows the one-way data binding pattern:
/// - Props flow down (immutable references from parent)
/// - Events flow up (actions returned in Output)
/// - Sidebar has its own state for child components
pub struct Sidebar {
    // Child components that Sidebar fully controls
    recent_files: RecentFiles,
    search: Search,
    bookmarks: Bookmarks,

    data_source_panel: HashMap<String, DataSourcePanel>,
    chart_studio: ChartStudio,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            recent_files: RecentFiles,
            search: Search::default(),
            bookmarks: Bookmarks::default(),
            data_source_panel: HashMap::new(),
            chart_studio: ChartStudio::default(),
        }
    }
}

/// Render a sidebar rail icon button and, when it's the selected section, paint
/// a 2px accent stripe down its left edge — the active-section indicator from
/// the design (RailButton: a `--primary` bar inset top/bottom at the left edge).
fn rail_button(ui: &mut egui::Ui, button: IconButton, accent: egui::Color32) -> bool {
    let selected = button.selected;
    let response = ui.add(button);
    if selected {
        let r = response.rect;
        let half = (r.height() * 0.34).min(16.0);
        let bar = egui::Rect::from_min_max(
            egui::pos2(r.min.x, r.center().y - half),
            egui::pos2(r.min.x + 2.5, r.center().y + half),
        );
        ui.painter().rect_filled(bar, 1.0, accent);
    }
    response.clicked()
}

impl Sidebar {
    /// Refresh the Chart Studio's eligible data-source list.
    pub fn set_chart_producers(&mut self, producers: Vec<ProducerRef>) {
        self.chart_studio.set_producers(producers);
    }

    /// Feed the resolved column schema for the selected chart source.
    pub fn set_chart_columns(&mut self, columns: Vec<ColumnInfo>) {
        self.chart_studio.set_columns(columns);
    }

    /// Preselect a Chart Studio data source (used by the "open in Charts" action).
    pub fn select_chart_source(&mut self, tab_id: TabId) {
        self.chart_studio.select_source(tab_id);
    }

    /// Update the Chart Studio's "Open Charts" list.
    pub fn set_chart_open(&mut self, open: Vec<(TabId, String)>) {
        self.chart_studio.set_open_charts(open);
    }

    /// Load an existing chart into the Chart Studio for editing.
    pub fn edit_chart(&mut self, spec: ChartSpec, columns: Vec<ColumnInfo>) {
        self.chart_studio.edit(spec, columns);
    }

    /// Lazily initialise a panel for `plugin_id` with the given loader.
    /// No-op if the panel already exists and has a loader (avoids resetting an active session).
    pub fn init_data_source_panel(
        &mut self,
        plugin_id: String,
        loader: crate::plugin::wasm_data_source::WasmDataSourceLoader,
    ) {
        let panel = self.data_source_panel.entry(plugin_id).or_default();
        if !panel.has_loader() {
            panel.set_loader(loader);
        }
    }

    /// Render the content area (when expanded)
    fn render_content(
        &mut self,
        ui: &mut egui::Ui,
        props: &SidebarProps<'_>,
        events: &mut Vec<SidebarEvent>,
    ) {
        // Render content based on selected section
        match &props.selected_section {
            Some(SidebarSection::RecentFiles) => {
                let output = self.recent_files.render(
                    ui,
                    RecentFilesProps {
                        recent_files: props.recent_files,
                    },
                );

                // Convert RecentFilesEvent to SidebarEvent
                for event in output.events {
                    match event {
                        RecentFilesEvent::OpenFile(path) => {
                            events.push(SidebarEvent::OpenFile(path));
                        }
                        RecentFilesEvent::RemoveFile(path) => {
                            events.push(SidebarEvent::RemoveRecentFile(path));
                        }
                        RecentFilesEvent::OpenFilePicker => {
                            events.push(SidebarEvent::OpenFilePicker);
                        }
                    }
                }
            }
            Some(SidebarSection::Search) => {
                self.render_search_section(ui, props, events);
            }
            Some(SidebarSection::Bookmarks) => {
                let output = self.bookmarks.render(
                    ui,
                    BookmarksProps {
                        bookmarks: props.bookmarks,
                        current_file_path: props.current_file_path,
                    },
                );

                // Convert BookmarksEvent to SidebarEvent
                for event in output.events {
                    match event {
                        BookmarksEvent::NavigateToBookmark { file_path, path } => {
                            events.push(SidebarEvent::NavigateToBookmark { file_path, path });
                        }
                        BookmarksEvent::JumpToPath(path) => {
                            events.push(SidebarEvent::JumpToPath(path));
                        }
                    }
                }
            }
            Some(SidebarSection::DataSource { plugin_id }) => {
                if let Some(panel) = self.data_source_panel.get_mut(plugin_id.as_str()) {
                    for ev in panel.render(ui, DataSourcePanelProps {}) {
                        match ev {
                            DataSourcePanelEvent::QueryResult { json, display_url } => {
                                events.push(SidebarEvent::DataSourceQueryResult {
                                    json,
                                    display_url,
                                });
                            }
                            DataSourcePanelEvent::ConsentNeeded(cr) => {
                                events.push(SidebarEvent::DataSourceConsentNeeded(cr));
                            }
                            DataSourcePanelEvent::Error(e) => {
                                events.push(SidebarEvent::DataSourceError(e));
                            }
                        }
                    }
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label(format!("Unable to load ui for {}", plugin_id));
                    });
                }
            }
            Some(SidebarSection::PluginSidebar { .. }) => {
                if let Some(info) = &props.plugin_sidebar {
                    match serde_json::from_str::<crate::plugin::render_node::UiNode>(
                        &info.output.node_json,
                    ) {
                        Ok(mut node) => {
                            let mut ui_events = Vec::new();
                            render_ui_node(ui, &mut node, &mut ui_events);
                            for evt in ui_events {
                                events.push(SidebarEvent::PluginSidebarEvent(evt));
                            }
                        }
                        Err(e) => {
                            ui.label(format!("Sidebar render error: {e}"));
                        }
                    }
                }
            }
            Some(SidebarSection::MarketPlace) => {
                Marketplace::render(ui, MarketplaceProps);
            }
            Some(SidebarSection::ChartStudio) => {
                for ev in self.chart_studio.render(ui) {
                    match ev {
                        ChartStudioEvent::SelectSource(id) => {
                            events.push(SidebarEvent::ChartSelectSource(id));
                        }
                        ChartStudioEvent::Generate(spec) => {
                            events.push(SidebarEvent::ChartGenerate(spec));
                        }
                        ChartStudioEvent::FocusChart(id) => {
                            events.push(SidebarEvent::ChartFocus(id));
                        }
                    }
                }
            }
            None => {}
        }
    }

    /// Render the icon buttons (always visible)
    fn render_icon_buttons(
        &mut self,
        ui: &mut egui::Ui,
        props: &SidebarProps<'_>,
        events: &mut Vec<SidebarEvent>,
    ) {
        let button_size = 48.0_f32;

        let accent = ui
            .ctx()
            .memory(|mem| {
                mem.data
                    .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
            })
            .map(|c| c.accent)
            .unwrap_or_else(|| ui.visuals().selection.bg_fill);

        let sidebar_btn = |icon: &str, tooltip: &str, selected: bool| {
            IconButton::builder()
                .icon(icon)
                .tooltip(tooltip)
                .size_px(button_size)
                .icon_size(20.0)
                .selected(selected)
                .build()
        };

        if rail_button(
            ui,
            sidebar_btn(
                egui_phosphor::regular::FOLDER,
                "Recent Files",
                props.selected_section == Some(SidebarSection::RecentFiles),
            ),
            accent,
        ) {
            events.push(SidebarEvent::SectionToggled(SidebarSection::RecentFiles));
        }

        if rail_button(
            ui,
            sidebar_btn(
                egui_phosphor::regular::MAGNIFYING_GLASS,
                "Search",
                props.selected_section == Some(SidebarSection::Search),
            ),
            accent,
        ) {
            events.push(SidebarEvent::SectionToggled(SidebarSection::Search));
        }

        if rail_button(
            ui,
            sidebar_btn(
                egui_phosphor::regular::BOOKMARK_SIMPLE,
                "Bookmarks",
                props.selected_section == Some(SidebarSection::Bookmarks),
            ),
            accent,
        ) {
            events.push(SidebarEvent::SectionToggled(SidebarSection::Bookmarks));
        }

        if rail_button(
            ui,
            sidebar_btn(
                egui_phosphor::regular::CHART_LINE,
                "Chart Studio",
                props.selected_section == Some(SidebarSection::ChartStudio),
            ),
            accent,
        ) {
            events.push(SidebarEvent::SectionToggled(SidebarSection::ChartStudio));
        }

        if rail_button(
            ui,
            sidebar_btn(
                egui_phosphor::regular::STOREFRONT,
                "Marketplace",
                props.selected_section == Some(SidebarSection::MarketPlace),
            ),
            accent,
        ) {
            events.push(SidebarEvent::SectionToggled(SidebarSection::MarketPlace));
        }

        // Plugin icons in a scroll area capped to leave room for the settings button,
        // so settings is never pushed off screen regardless of how many plugins exist.
        let plugins_max_h = (ui.available_height() - button_size).max(0.0);
        egui::ScrollArea::vertical()
            .id_salt("sidebar_plugin_icons")
            .max_height(plugins_max_h)
            .show(ui, |ui| {
                for plugin in props.data_source_plugins {
                    let section = SidebarSection::DataSource {
                        plugin_id: plugin.id.clone(),
                    };
                    // Highlight only while this plugin's sidebar is the open
                    // section — not merely because it has an active session
                    // (otherwise the icon stays selected after switching away).
                    // A data-source plugin's open section is tracked as
                    // `PluginSidebar { plugin_id }`, so match against that.
                    let selected = matches!(
                        &props.selected_section,
                        Some(SidebarSection::PluginSidebar { plugin_id: p }) if p == &plugin.id
                    );
                    let icon = plugin
                        .icon
                        .as_deref()
                        .unwrap_or(egui_phosphor::regular::DATABASE);
                    if rail_button(
                        ui,
                        IconButton::builder()
                            .icon(icon)
                            .tooltip(plugin.name.as_str())
                            .size_px(button_size)
                            .icon_size(20.0)
                            .selected(selected)
                            .build(),
                        accent,
                    ) {
                        events.push(SidebarEvent::SectionToggled(section));
                    }
                }

                // Pure ui-component plugins — clicking opens the plugin in a new tab.
                for plugin in props.ui_component_plugins {
                    let icon = plugin
                        .icon
                        .as_deref()
                        .unwrap_or(egui_phosphor::regular::PUZZLE_PIECE);
                    if ui
                        .add(
                            IconButton::builder()
                                .icon(icon)
                                .tooltip(plugin.name.as_str())
                                .size_px(button_size)
                                .icon_size(20.0)
                                .build(),
                        )
                        .clicked()
                    {
                        events.push(SidebarEvent::OpenUiComponentTab(plugin.id.clone()));
                    }
                }
            });

        // Push settings to the absolute bottom of the strip
        let remaining = ui.available_height();
        if remaining > button_size {
            ui.add_space(remaining - button_size);
        }

        // Settings icon pinned to the bottom of the icon strip
        if ui
            .add(sidebar_btn(egui_phosphor::regular::GEAR, "Settings", false))
            .clicked()
        {
            events.push(SidebarEvent::OpenSettings);
        }
    }

    fn render_search_section(
        &mut self,
        ui: &mut egui::Ui,
        props: &SidebarProps<'_>,
        events: &mut Vec<SidebarEvent>,
    ) {
        // Render the Search component using the trait method
        // Parent determines when to focus via props.focus_search
        let search_output = self.search.render(
            ui,
            SearchProps {
                just_opened: props.focus_search,
                search_state: props.search_state,
                search_history: props.search_history,
            },
        );

        // Convert SearchEvent to SidebarEvent
        for event in search_output.events {
            match event {
                SearchEvent::Search(msg) => events.push(SidebarEvent::Search(msg)),
                SearchEvent::NavigateToResult { record_index } => {
                    events.push(SidebarEvent::NavigateToSearchResult { record_index })
                }
                SearchEvent::ClearHistory => events.push(SidebarEvent::ClearSearchHistory),
            }
        }
    }
}

impl ContextComponent for Sidebar {
    type Props<'a> = SidebarProps<'a>;
    type Output = SidebarOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let mut events = Vec::new();

        let colors = ui
            .ctx()
            .memory(|mem| {
                mem.data
                    .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
            })
            .unwrap_or_else(|| {
                let dark_mode = ui.ctx().global_style().visuals.dark_mode;
                crate::theme::Theme::for_dark_mode(dark_mode).colors()
            });

        let panel_bg = colors.bg_panel;

        // Layout constants
        const LEFT_PAD: f32 = 10.0; // gap from window edge to rail
        const RAIL_W: f32 = 48.0;
        const INNER_GAP: f32 = crate::theme::GUTTER_GAP; // gap between rail and content
        const RIGHT_GAP: f32 = crate::theme::GUTTER_GAP; // gap from sidebar to dock

        // Transparent outer container — no background, no border. It purely
        // allocates space in the panel system; the visual chrome lives on the
        // inner rail and content frames painted via paint_at.
        let mut outer_panel = egui::Panel::left("sidebar");
        if props.expanded {
            let min_w = LEFT_PAD + RAIL_W + INNER_GAP + MIN_SIDEBAR_WIDTH + RIGHT_GAP;
            let window_width = ui.ctx().content_rect().width();
            let max_w = window_width * MAX_SIDEBAR_WIDTH_RATIO;
            outer_panel = outer_panel
                .resizable(true)
                .size_range(min_w..=max_w)
                .default_size(props.sidebar_width.clamp(min_w, max_w));
        } else {
            outer_panel = outer_panel
                .resizable(false)
                .exact_size(LEFT_PAD + RAIL_W + RIGHT_GAP);
        }

        let outer_response = outer_panel
            .show_separator_line(false)
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                let available = ui.available_rect_before_wrap();
                let h = available.height();

                // Shared floating frame style (fill, radius, shadow, stroke).
                // paint_at uses the panel's painter so shadow bleeds into the
                // gap/crust area without being clipped by child UI clip rects.
                let floating = egui::Frame::NONE
                    .fill(panel_bg)
                    .corner_radius(egui::CornerRadius::same(crate::theme::RADIUS_PANEL as u8))
                    .shadow(crate::theme::panel_shadow(ui.visuals().dark_mode))
                    .stroke(crate::theme::edge_stroke(&colors));

                // Leave GUTTER_GAP crust gap at the bottom so panel shadows fall
                // into the gap rather than bleeding into the transparent status bar.
                let panel_h = h - RIGHT_GAP; // RIGHT_GAP == GUTTER_GAP

                // ── Rail ─────────────────────────────────────────────────────
                let rail_rect = egui::Rect::from_min_size(
                    available.min + egui::vec2(LEFT_PAD, 0.0),
                    egui::vec2(RAIL_W, panel_h),
                );
                ui.painter().add(floating.paint(rail_rect));
                let mut rail_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(rail_rect)
                        .layout(egui::Layout::top_down(egui::Align::Center)),
                );
                self.render_icon_buttons(&mut rail_ui, &props, &mut events);

                // ── Content (when expanded) ───────────────────────────────────
                if props.expanded {
                    let content_x = LEFT_PAD + RAIL_W + INNER_GAP;
                    let content_w = (available.width() - content_x - RIGHT_GAP).max(0.0);
                    if content_w > 0.0 {
                        let content_rect = egui::Rect::from_min_size(
                            available.min + egui::vec2(content_x, 0.0),
                            egui::vec2(content_w, panel_h),
                        );
                        ui.painter().add(floating.paint(content_rect));
                        let mut content_ui = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(content_rect)
                                .layout(egui::Layout::top_down(egui::Align::Min)),
                        );
                        egui::ScrollArea::vertical().show(&mut content_ui, |ui| {
                            self.render_content(ui, &props, &mut events);
                        });
                    }
                }

                ui.allocate_rect(available, egui::Sense::hover());
            });

        if props.expanded {
            let actual_width = outer_response.response.rect.width();
            if (actual_width - props.sidebar_width).abs() > 0.1 {
                events.push(SidebarEvent::WidthChanged(actual_width));
            }
        }

        SidebarOutput { events }
    }
}
