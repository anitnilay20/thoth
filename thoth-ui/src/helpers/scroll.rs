use eframe::egui::{self, Ui};
use std::ops::Range;

use crate::constants::SCROLL_MARGIN;

/// Handles incremental scrolling for search navigation that persists across frames.
///
/// This function uses `scroll_with_delta` combined with `request_repaint` to scroll
/// incrementally toward a target row, overcoming egui's per-frame scroll delta limit.
///
/// Returns `true` if the target has been reached (and should be cleared), `false` otherwise.
///
/// # Arguments
/// * `ui` - The egui UI context
/// * `row_range` - The range of currently visible rows
/// * `target_row` - The target row index to scroll to
/// * `row_height` - The height of each row in pixels
pub fn scroll_to_search_target(
    ui: &mut Ui,
    row_range: &Range<usize>,
    target_row: usize,
    row_height: f32,
) -> bool {
    // Calculate the ideal position: target row should be a few rows from the top
    // This gives a small amount of context above the target
    let ideal_offset_from_top = 2;
    let ideal_top_row = target_row.saturating_sub(ideal_offset_from_top);

    // Check if we're close enough to the ideal position (within 2 rows tolerance)
    let tolerance = 2;
    let current_distance = (row_range.start as i32 - ideal_top_row as i32).abs();

    if current_distance > tolerance {
        // Calculate delta to scroll to ideal position
        let target_offset = (ideal_top_row as f32) * row_height;
        let current_offset = (row_range.start as f32) * row_height;
        let delta_y = -(target_offset - current_offset);

        ui.scroll_with_delta(egui::vec2(0.0, delta_y));
        ui.ctx().request_repaint(); // Request another frame to continue scrolling
        false // Not reached yet
    } else {
        // Target is at ideal position
        true // Reached
    }
}

/// Automatically scrolls the view to keep the selected item visible when navigating with keyboard.
///
/// This function implements smooth scrolling behavior with margins: when the selection moves
/// near the top or bottom edge of the visible area, the view scrolls to maintain context.
/// The scroll is only triggered once per selection change (controlled by `should_scroll_to_selection`).
///
/// After scrolling, the caller should reset `should_scroll_to_selection` to prevent continuous
/// scrolling on subsequent frames.
///
/// # Arguments
/// * `ui` - The egui UI context
/// * `row_range` - The range of currently visible rows
/// * `current_index` - The index of the currently selected item
/// * `row_height` - The height of each row in pixels
/// * `should_scroll_to_selection` - Whether to scroll to the selected item (mutable flag)
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

    let scroll_margin: usize =
        if current_index > row_range.end && current_index - row_range.end >= SCROLL_MARGIN {
            current_index
        } else {
            SCROLL_MARGIN
        };

    // Scrolling down: when near the bottom of visible range
    if current_index >= row_range.end.saturating_sub(scroll_margin) {
        // Relative scrolling down: scroll to current row + 1
        let target_y = ui.max_rect().height() + (scroll_margin as f32 * row_height);
        ui.scroll_to_rect(
            egui::Rect::from_min_size(
                egui::pos2(0.0, target_y),
                egui::vec2(ui.available_width(), row_height),
            ),
            None,
        );
    }
    // Scrolling up: when near the top of visible range
    else if current_index < row_range.start + scroll_margin {
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
