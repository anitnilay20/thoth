use crate::components::traits::StatelessComponent;
use eframe::egui;

/// Props passed to the Breadcrumbs component
pub struct BreadcrumbsProps<'a> {
    /// Current selected path (e.g., "0.user.items[2].name")
    pub current_path: Option<&'a str>,
}

/// Events emitted by the Breadcrumbs component
#[derive(Debug, Clone)]
pub enum BreadcrumbsEvent {
    /// User clicked on a breadcrumb segment to navigate to that path
    NavigateToPath(String),
}

pub struct BreadcrumbsOutput {
    pub events: Vec<BreadcrumbsEvent>,
}

/// Breadcrumbs navigation component
///
/// Displays the current path as clickable segments (e.g., "Root > users > [0] > name")
/// Each segment is clickable to navigate to that level in the JSON hierarchy
pub struct Breadcrumbs;

impl StatelessComponent for Breadcrumbs {
    type Props<'a> = BreadcrumbsProps<'a>;
    type Output = BreadcrumbsOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let mut events = Vec::new();

        // Get theme colors
        let (text_color, muted_color) = ui.ctx().memory(|mem| {
            if let Some(colors) = mem
                .data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
            {
                (colors.text, colors.overlay1)
            } else {
                // Fallback colors
                (
                    egui::Color32::from_rgb(204, 204, 204),
                    egui::Color32::from_rgb(128, 128, 128),
                )
            }
        });

        // Early return if no path selected
        let Some(path) = props.current_path else {
            // Show "No selection" message
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("No selection")
                        .size(12.0)
                        .color(muted_color),
                );
            });
            return BreadcrumbsOutput { events };
        };

        if path.is_empty() {
            // Show root
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Root").size(12.0).color(text_color));
            });
            return BreadcrumbsOutput { events };
        }

        // Parse path into segments
        let segments = Self::parse_path(path);

        // Render breadcrumbs
        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // Root is always clickable
            if ui
                .link(egui::RichText::new("Root").size(12.0).color(text_color))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                events.push(BreadcrumbsEvent::NavigateToPath(String::new()));
            }

            // Render each segment
            for (i, segment) in segments.iter().enumerate() {
                // Separator
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::CARET_RIGHT)
                        .size(10.0)
                        .color(muted_color),
                );

                // Build path up to this segment
                let segment_path = segments[..=i].join(".");

                // Last segment is not clickable (current location)
                if i == segments.len() - 1 {
                    ui.label(
                        egui::RichText::new(segment)
                            .size(12.0)
                            .color(text_color)
                            .strong(),
                    );
                } else {
                    // Clickable segment
                    let mut response = ui
                        .link(egui::RichText::new(segment).size(12.0).color(text_color))
                        .on_hover_cursor(egui::CursorIcon::PointingHand);

                    response = response.on_hover_text(format!("Navigate to {}", segment_path));

                    if response.clicked() {
                        events.push(BreadcrumbsEvent::NavigateToPath(segment_path));
                    }
                }
            }

            ui.add_space(8.0);
        });

        BreadcrumbsOutput { events }
    }
}

impl Breadcrumbs {
    /// Parse a path string into displayable segments
    ///
    /// Examples:
    /// - "0.user.name" -> ["[0]", "user", "name"]
    /// - "items[2].title" -> ["items", "[2]", "title"]
    fn parse_path(path: &str) -> Vec<String> {
        let mut segments = Vec::new();
        let mut current = String::new();

        for ch in path.chars() {
            match ch {
                '.' => {
                    if !current.is_empty() {
                        segments.push(current.clone());
                        current.clear();
                    }
                }
                _ => current.push(ch),
            }
        }

        // Push last segment
        if !current.is_empty() {
            segments.push(current);
        }

        // Format array indices with brackets if they're just numbers
        segments
            .into_iter()
            .map(|s| {
                if s.chars().all(|c| c.is_ascii_digit()) {
                    format!("[{}]", s)
                } else {
                    s
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_path_simple() {
        let segments = Breadcrumbs::parse_path("0.user.name");
        assert_eq!(segments, vec!["[0]", "user", "name"]);
    }

    #[test]
    fn test_parse_path_array_indices() {
        let segments = Breadcrumbs::parse_path("items.2.title");
        assert_eq!(segments, vec!["items", "[2]", "title"]);
    }

    #[test]
    fn test_parse_path_mixed() {
        let segments = Breadcrumbs::parse_path("data.0.items.1.value");
        assert_eq!(segments, vec!["data", "[0]", "items", "[1]", "value"]);
    }

    #[test]
    fn test_parse_path_single_segment() {
        let segments = Breadcrumbs::parse_path("users");
        assert_eq!(segments, vec!["users"]);
    }

    #[test]
    fn test_parse_path_single_index() {
        let segments = Breadcrumbs::parse_path("0");
        assert_eq!(segments, vec!["[0]"]);
    }

    #[test]
    fn test_parse_path_empty() {
        let segments = Breadcrumbs::parse_path("");
        assert_eq!(segments, Vec::<String>::new());
    }

    #[test]
    fn test_parse_path_trailing_dot() {
        let segments = Breadcrumbs::parse_path("user.name.");
        assert_eq!(segments, vec!["user", "name"]);
    }

    #[test]
    fn test_parse_path_multiple_dots() {
        let segments = Breadcrumbs::parse_path("a..b");
        assert_eq!(segments, vec!["a", "b"]);
    }

    #[test]
    fn test_breadcrumbs_event_debug() {
        let event = BreadcrumbsEvent::NavigateToPath("test".to_string());
        assert!(format!("{:?}", event).contains("NavigateToPath"));
    }

    #[test]
    fn test_breadcrumbs_event_clone() {
        let event = BreadcrumbsEvent::NavigateToPath("test".to_string());
        let cloned = event.clone();
        assert!(matches!(cloned, BreadcrumbsEvent::NavigateToPath(_)));
    }
}
