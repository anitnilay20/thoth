use eframe::egui::{self, Ui};
use std::ops::Range;

/// Scroll to the currently selected item in a list view
///
/// # Arguments
/// * `ui` - The egui UI context
/// * `row_range` - The range of currently visible rows
/// * `current_index` - The index of the currently selected item
/// * `row_height` - The height of each row in pixels
/// * `should_scroll_to_selection` - Whether to scroll to the selected item (mutable, will be reset after scrolling)
pub fn scroll_to_selection(
    ui: &mut Ui,
    row_range: &Range<usize>,
    current_index: usize,
    row_height: f32,
    should_scroll_to_selection: &mut bool,
) {
    if !*should_scroll_to_selection {
        return;
    }

    const SCROLL_MARGIN: usize = 3; // Number of rows margin before scrolling

    // Scrolling down: when near the bottom of visible range
    if current_index >= row_range.end.saturating_sub(SCROLL_MARGIN) {
        // Relative scrolling down: scroll to current row + 1
        let target_y = ui.max_rect().height() + (SCROLL_MARGIN as f32 * row_height);
        ui.scroll_to_rect(
            egui::Rect::from_min_size(
                egui::pos2(0.0, target_y),
                egui::vec2(ui.available_width(), row_height),
            ),
            None,
        );
    }
    // Scrolling up: when near the top of visible range
    else if current_index < row_range.start + SCROLL_MARGIN {
        // Scroll up by one row height (relative scrolling)
        let current_pos = ui.cursor().top();
        ui.scroll_to_rect(
            egui::Rect::from_min_size(
                egui::pos2(0.0, (current_pos - row_height).max(0.0)),
                egui::vec2(ui.available_width(), row_height),
            ),
            None,
        );
    }
}
