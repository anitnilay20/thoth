use crate::components::icon_button::{IconButton, IconButtonProps};
use crate::components::traits::{StatefulComponent, StatelessComponent};
use crate::search::SearchMessage;
use eframe::egui;

/// Props passed to the Search panel (immutable, one-way binding)
pub struct SearchProps {
    /// Whether this is the first render since the panel was opened
    pub just_opened: bool,
}

/// Events emitted by the Search panel
pub enum SearchEvent {
    Search(SearchMessage),
}

pub struct SearchOutput {
    pub events: Vec<SearchEvent>,
}

/// Stateful search panel component for sidebar
#[derive(Default)]
pub struct Search {
    search_query: String,
    match_case: bool,
}

impl StatefulComponent for Search {
    type Props<'a> = SearchProps;
    type Output = SearchOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();

        // Get theme colors for header
        let theme_colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
        });

        let (header_color, _, input_bg) = if let Some(colors) = theme_colors {
            (colors.sidebar_header, colors.text, colors.surface0)
        } else {
            (
                egui::Color32::from_rgb(153, 153, 153),
                egui::Color32::from_rgb(204, 204, 204),
                egui::Color32::from_rgb(49, 50, 68),
            )
        };

        // Header with buttons
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("SEARCH")
                    .size(11.0)
                    .color(header_color)
                    .strong(),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Clear button
                let clear_output = IconButton::render(
                    ui,
                    IconButtonProps {
                        icon: egui_phosphor::regular::X,
                        frame: false,
                        tooltip: Some("Clear search"),
                        badge_color: None,
                        size: None,
                    },
                );
                if clear_output.clicked {
                    self.search_query.clear();
                    if let Some(msg) = SearchMessage::create_search(String::new(), self.match_case)
                    {
                        events.push(SearchEvent::Search(msg));
                    }
                }

                // Search button
                let search_output = IconButton::render(
                    ui,
                    IconButtonProps {
                        icon: egui_phosphor::regular::MAGNIFYING_GLASS,
                        frame: false,
                        tooltip: Some("Search"),
                        badge_color: None,
                        size: None,
                    },
                );
                if search_output.clicked && !self.search_query.is_empty() {
                    if let Some(msg) =
                        SearchMessage::create_search(self.search_query.clone(), self.match_case)
                    {
                        events.push(SearchEvent::Search(msg));
                    }
                }
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(8.0);

        // Search input field with custom background
        let text_edit = egui::TextEdit::singleline(&mut self.search_query)
            .desired_width(f32::INFINITY)
            .hint_text("Search...");

        // Apply custom background color to make the input more visible
        let response = ui.add(text_edit.background_color(input_bg));

        // Auto-focus only when the panel is just opened
        if props.just_opened {
            response.request_focus();
        }

        // Add accessibility info for screen readers
        response.widget_info(|| {
            egui::WidgetInfo::text_edit(
                ui.is_enabled(),
                &self.search_query,
                &self.search_query,
                "Search...",
            )
        });

        // Handle Enter key to trigger search
        if response.lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter))
            && !self.search_query.is_empty()
        {
            if let Some(msg) =
                SearchMessage::create_search(self.search_query.clone(), self.match_case)
            {
                events.push(SearchEvent::Search(msg));
            }
        }

        ui.add_space(8.0);

        // Match case checkbox with accessibility info
        let checkbox_response = ui.checkbox(&mut self.match_case, "Match case");
        checkbox_response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                ui.is_enabled(),
                self.match_case,
                "Match case",
            )
        });

        SearchOutput { events }
    }
}
