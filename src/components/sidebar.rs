use crate::components::recent_files::{RecentFiles, RecentFilesEvent, RecentFilesProps};
use crate::components::search::{Search, SearchEvent, SearchProps};
use crate::components::settings_panel::{SettingsPanel, SettingsPanelEvent, SettingsPanelProps};
use crate::components::traits::{ContextComponent, StatefulComponent};
use crate::constants::{MAX_SIDEBAR_WIDTH_RATIO, MIN_SIDEBAR_WIDTH};
use crate::search::SearchMessage;
use eframe::egui;

/// Which sidebar section is currently selected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSection {
    RecentFiles,
    Search,
    Settings,
}

/// Props passed to the Sidebar (immutable, one-way binding)
pub struct SidebarProps<'a> {
    pub recent_files: &'a [String],
    pub expanded: bool,
    pub sidebar_width: f32,
    pub selected_section: Option<SidebarSection>,
    /// Whether the search section should receive focus (when just opened)
    pub focus_search: bool,
    /// Update status for settings panel
    pub update_status: &'a crate::update::UpdateStatus,
    /// Current version for settings panel
    pub current_version: &'a str,
    /// Current search state with results
    pub search_state: &'a crate::search::Search,
    /// Search history for the current file
    pub search_history: Option<&'a Vec<String>>,
}

/// Events emitted by the Sidebar
#[derive(Debug, Clone)]
pub enum SidebarEvent {
    OpenFile(String),
    RemoveRecentFile(String),
    OpenFilePicker,
    SectionToggled(SidebarSection),
    WidthChanged(f32),
    // Search events
    Search(SearchMessage),
    NavigateToSearchResult { record_index: usize },
    ClearSearchHistory,
    // Settings events
    CheckForUpdates,
    DownloadUpdate,
    InstallUpdate,
    RetryUpdate,
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
    settings_panel: SettingsPanel,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            recent_files: RecentFiles,
            search: Search::default(),
            settings_panel: SettingsPanel,
        }
    }
}

impl Sidebar {
    /// Render the content area (when expanded)
    fn render_content(
        &mut self,
        ui: &mut egui::Ui,
        props: &SidebarProps<'_>,
        events: &mut Vec<SidebarEvent>,
    ) {
        // Render content based on selected section
        match props.selected_section {
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
            Some(SidebarSection::Settings) => {
                self.render_settings_section(ui, props, events);
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
        hover_bg: egui::Color32,
        selection_bg: egui::Color32,
        text_color: egui::Color32,
    ) {
        let icon_size = 20.0;
        let button_size = egui::vec2(48.0, 48.0);

        // Recent Files button
        let recent_files_selected = props.selected_section == Some(SidebarSection::RecentFiles);
        if self.render_icon_button(
            ui,
            egui_phosphor::regular::FOLDER,
            "Recent Files",
            recent_files_selected,
            (button_size, icon_size),
            (hover_bg, selection_bg, text_color),
        ) {
            // Emit toggle event - parent will decide whether to collapse or expand
            events.push(SidebarEvent::SectionToggled(SidebarSection::RecentFiles));
        }

        // Search button
        let search_selected = props.selected_section == Some(SidebarSection::Search);
        if self.render_icon_button(
            ui,
            egui_phosphor::regular::MAGNIFYING_GLASS,
            "Search",
            search_selected,
            (button_size, icon_size),
            (hover_bg, selection_bg, text_color),
        ) {
            // Emit toggle event - parent will decide whether to collapse or expand
            events.push(SidebarEvent::SectionToggled(SidebarSection::Search));
        }

        // Settings button
        let settings_selected = props.selected_section == Some(SidebarSection::Settings);
        if self.render_icon_button(
            ui,
            egui_phosphor::regular::GEAR,
            "Settings",
            settings_selected,
            (button_size, icon_size),
            (hover_bg, selection_bg, text_color),
        ) {
            // Emit toggle event - parent will decide whether to collapse or expand
            events.push(SidebarEvent::SectionToggled(SidebarSection::Settings));
        }
    }

    fn render_icon_button(
        &self,
        ui: &mut egui::Ui,
        icon: &str,
        tooltip: &str,
        selected: bool,
        (size, icon_size): (egui::Vec2, f32),
        (hover_bg, selection_bg, text_color): (egui::Color32, egui::Color32, egui::Color32),
    ) -> bool {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            // Background
            if selected || response.hovered() {
                let bg_color = if selected {
                    selection_bg // Use theme selection color
                } else {
                    hover_bg
                };
                ui.painter().rect_filled(rect, 0.0, bg_color);
            }

            // Icon (always use text_color)
            let icon_color = text_color;

            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                icon,
                egui::FontId::proportional(icon_size),
                icon_color,
            );
        }

        response.on_hover_text(tooltip).clicked()
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

    fn render_settings_section(
        &mut self,
        ui: &mut egui::Ui,
        props: &SidebarProps<'_>,
        events: &mut Vec<SidebarEvent>,
    ) {
        // Render the SettingsPanel component
        let settings_output = self.settings_panel.render(
            ui,
            SettingsPanelProps {
                update_status: props.update_status,
                current_version: props.current_version,
            },
        );

        // Convert SettingsPanelEvent to SidebarEvent
        for event in settings_output.events {
            match event {
                SettingsPanelEvent::CheckForUpdates => {
                    events.push(SidebarEvent::CheckForUpdates);
                }
                SettingsPanelEvent::DownloadUpdate => {
                    events.push(SidebarEvent::DownloadUpdate);
                }
                SettingsPanelEvent::InstallUpdate => {
                    events.push(SidebarEvent::InstallUpdate);
                }
                SettingsPanelEvent::RetryUpdate => {
                    events.push(SidebarEvent::RetryUpdate);
                }
            }
        }
    }
}

impl ContextComponent for Sidebar {
    type Props<'a> = SidebarProps<'a>;
    type Output = SidebarOutput;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let mut events = Vec::new();

        // Get theme colors
        let theme_colors = ctx.memory(|mem| {
            mem.data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
        });

        let (icon_strip_bg, content_bg, hover_bg, selection_bg, text_color) =
            if let Some(colors) = theme_colors {
                (
                    colors.crust,  // Icon strip uses darker crust
                    colors.mantle, // Content area uses mantle
                    colors.sidebar_hover,
                    colors.overlay1, // Selection background
                    colors.text,
                )
            } else {
                // Fallback colors
                (
                    egui::Color32::from_rgb(30, 30, 30),
                    egui::Color32::from_rgb(37, 37, 38),
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 13),
                    egui::Color32::from_rgb(60, 60, 60),
                    egui::Color32::from_rgb(204, 204, 204),
                )
            };

        let sidebar_width = if props.expanded {
            props.sidebar_width
        } else {
            48.0
        };

        // Build sidebar panel - always resizable when expanded
        let is_resizable = props.expanded;
        let mut sidebar_panel = egui::SidePanel::left("sidebar").resizable(is_resizable);

        // Set width constraints
        if props.expanded {
            // When expanded, use stored width with min/max constraints
            let min_width = MIN_SIDEBAR_WIDTH;
            let window_width = ctx.screen_rect().width();
            let max_width = window_width * MAX_SIDEBAR_WIDTH_RATIO;

            sidebar_panel = sidebar_panel
                .resizable(true)
                .width_range(min_width..=max_width)
                .default_width(props.sidebar_width.clamp(min_width, max_width));
        } else {
            // When collapsed, use icon strip width
            sidebar_panel = sidebar_panel.exact_width(sidebar_width);
        }

        let sidebar_response = sidebar_panel
            .frame(egui::Frame::NONE.fill(if props.expanded {
                content_bg // Use content background, icon strip will override its area
            } else {
                icon_strip_bg
            }))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                if props.expanded {
                    // Horizontal layout: icon buttons on left, content on right
                    let available_rect = ui.available_rect_before_wrap();
                    let available_height = available_rect.height();
                    let actual_width = available_rect.width(); // Use actual rendered width

                    // Left side: 48px icon strip with darker background
                    let icon_strip_rect = egui::Rect::from_min_size(
                        available_rect.min,
                        egui::vec2(48.0, available_height),
                    );
                    ui.painter()
                        .rect_filled(icon_strip_rect, 0.0, icon_strip_bg);

                    let mut icon_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(icon_strip_rect)
                            .layout(egui::Layout::top_down(egui::Align::Center)),
                    );
                    self.render_icon_buttons(
                        &mut icon_ui,
                        &props,
                        &mut events,
                        hover_bg,
                        selection_bg,
                        text_color,
                    );

                    // Right side: expanded content (takes remaining width)
                    let content_width = actual_width - 48.0; // Calculate from actual width
                    let content_rect = egui::Rect::from_min_size(
                        available_rect.min + egui::vec2(48.0, 0.0),
                        egui::vec2(content_width, available_height),
                    );

                    let mut content_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(content_rect)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );

                    // Add frame with inner padding
                    egui::Frame::NONE.inner_margin(egui::Margin::same(8)).show(
                        &mut content_ui,
                        |ui| {
                            self.render_content(ui, &props, &mut events);
                        },
                    );

                    // Advance the cursor to consume the full area
                    ui.allocate_rect(available_rect, egui::Sense::hover());
                } else {
                    // Just show icon buttons
                    self.render_icon_buttons(
                        ui,
                        &props,
                        &mut events,
                        hover_bg,
                        selection_bg,
                        text_color,
                    );
                }
            });

        // Emit width change event if sidebar is being actively resized
        if props.expanded {
            let actual_width = sidebar_response.response.rect.width();
            if (actual_width - props.sidebar_width).abs() > 0.1 {
                events.push(SidebarEvent::WidthChanged(actual_width));
            }
        }

        SidebarOutput { events }
    }
}
