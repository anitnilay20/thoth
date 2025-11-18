/// macOS-specific title bar implementation with traffic light buttons
use eframe::egui::{self, Id, PointerButton, Sense, ViewportCommand};

use super::{TITLE_BAR_HEIGHT, TitleBarProps, title_bar_background, title_bar_text_color};

const TRAFFIC_LIGHT_RADIUS: f32 = 6.0;
const TRAFFIC_LIGHT_SPACING: f32 = 8.0;
const TRAFFIC_LIGHT_OFFSET_X: f32 = 12.0;

/// Render macOS-style title bar with traffic light buttons on the left
pub fn render_title_bar(ui: &mut egui::Ui, props: TitleBarProps<'_>) {
    let available_rect = ui.available_rect_before_wrap();
    let title_bar_rect = egui::Rect::from_min_size(
        available_rect.min,
        egui::vec2(ui.available_width(), TITLE_BAR_HEIGHT),
    );

    // Interact with the title bar for dragging
    let title_bar_response = ui.interact(
        title_bar_rect,
        Id::new("title_bar"),
        Sense::click_and_drag(),
    );

    // Start window drag on primary button drag
    if title_bar_response.drag_started_by(PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }

    // Double-click to maximize/restore
    if title_bar_response.double_clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(
            !ui.input(|i| i.viewport().maximized.unwrap_or(false)),
        ));
    }

    // Paint title bar background
    ui.painter()
        .rect_filled(title_bar_rect, 0.0, title_bar_background(props.dark_mode));

    // Reserve space for the title bar
    let title_bar_ui = &mut ui.new_child(
        egui::UiBuilder::new()
            .max_rect(title_bar_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );

    // Add spacing for traffic lights
    title_bar_ui.add_space(TRAFFIC_LIGHT_OFFSET_X);

    // Render traffic light buttons
    render_traffic_lights(title_bar_ui, &title_bar_response);

    // Center the title
    title_bar_ui.add_space(20.0);

    // Calculate centered position for title
    let title_text = egui::RichText::new(props.title)
        .size(13.0)
        .color(title_bar_text_color(props.dark_mode));

    title_bar_ui.label(title_text);

    // Draw bottom border
    let border_y = title_bar_rect.max.y;
    ui.painter().line_segment(
        [
            egui::pos2(title_bar_rect.min.x, border_y),
            egui::pos2(title_bar_rect.max.x, border_y),
        ],
        egui::Stroke::new(1.0, egui::Color32::from_rgb(0x3e, 0x3e, 0x42)),
    );

    // Allocate the space
    ui.allocate_space(egui::vec2(ui.available_width(), TITLE_BAR_HEIGHT));
}

/// Render macOS traffic light buttons (close, minimize, maximize)
fn render_traffic_lights(ui: &mut egui::Ui, title_bar_response: &egui::Response) {
    let button_size = egui::vec2(TRAFFIC_LIGHT_RADIUS * 2.0, TRAFFIC_LIGHT_RADIUS * 2.0);
    let hovered = title_bar_response.hovered();

    // Close button (red)
    let close_response = ui.allocate_rect(
        egui::Rect::from_min_size(ui.cursor().min, button_size),
        Sense::click(),
    );

    let close_color = if close_response.hovered() {
        egui::Color32::from_rgb(0xed, 0x6a, 0x5e) // Bright red on hover
    } else if hovered {
        egui::Color32::from_rgb(0xff, 0x5f, 0x57) // Red when title bar hovered
    } else {
        egui::Color32::from_rgb(0x4d, 0x4d, 0x4d) // Gray when not hovered
    };

    ui.painter().circle_filled(
        close_response.rect.center(),
        TRAFFIC_LIGHT_RADIUS,
        close_color,
    );

    if close_response.clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
    }

    ui.add_space(TRAFFIC_LIGHT_SPACING);

    // Minimize button (yellow)
    let minimize_response = ui.allocate_rect(
        egui::Rect::from_min_size(ui.cursor().min, button_size),
        Sense::click(),
    );

    let minimize_color = if minimize_response.hovered() {
        egui::Color32::from_rgb(0xf5, 0xbf, 0x4f) // Bright yellow on hover
    } else if hovered {
        egui::Color32::from_rgb(0xfe, 0xbc, 0x2e) // Yellow when title bar hovered
    } else {
        egui::Color32::from_rgb(0x4d, 0x4d, 0x4d) // Gray when not hovered
    };

    ui.painter().circle_filled(
        minimize_response.rect.center(),
        TRAFFIC_LIGHT_RADIUS,
        minimize_color,
    );

    if minimize_response.clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
    }

    ui.add_space(TRAFFIC_LIGHT_SPACING);

    // Maximize button (green)
    let maximize_response = ui.allocate_rect(
        egui::Rect::from_min_size(ui.cursor().min, button_size),
        Sense::click(),
    );

    let maximize_color = if maximize_response.hovered() {
        egui::Color32::from_rgb(0x62, 0xc4, 0x54) // Bright green on hover
    } else if hovered {
        egui::Color32::from_rgb(0x28, 0xc8, 0x40) // Green when title bar hovered
    } else {
        egui::Color32::from_rgb(0x4d, 0x4d, 0x4d) // Gray when not hovered
    };

    ui.painter().circle_filled(
        maximize_response.rect.center(),
        TRAFFIC_LIGHT_RADIUS,
        maximize_color,
    );

    if maximize_response.clicked() {
        let is_maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Maximized(!is_maximized));
    }
}
