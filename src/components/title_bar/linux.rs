/// Linux-specific title bar implementation with standard controls
use eframe::egui::{self, Id, PointerButton, Sense, ViewportCommand};

use super::{
    TITLE_BAR_HEIGHT, TitleBarEvent, TitleBarProps, title_bar_background, title_bar_text_color,
};

const BUTTON_SIZE: f32 = 32.0;
const BUTTON_SPACING: f32 = 4.0;

/// Render Linux-style title bar with controls on the right
pub fn render_title_bar(
    ui: &mut egui::Ui,
    props: TitleBarProps<'_>,
    events: &mut Vec<TitleBarEvent>,
) {
    let available_rect = ui.available_rect_before_wrap();
    let title_bar_rect = egui::Rect::from_min_size(
        available_rect.min,
        egui::vec2(ui.available_width(), TITLE_BAR_HEIGHT),
    );

    // Calculate non-interactive area (where buttons are)
    let button_area_width = (BUTTON_SIZE + BUTTON_SPACING) * 3.0 + 8.0;
    let draggable_rect = egui::Rect::from_min_size(
        title_bar_rect.min,
        egui::vec2(ui.available_width() - button_area_width, TITLE_BAR_HEIGHT),
    );

    // Interact with the draggable area only
    let title_bar_response = ui.interact(
        draggable_rect,
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

    // Create title bar layout
    let title_bar_ui = &mut ui.new_child(
        egui::UiBuilder::new()
            .max_rect(title_bar_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );

    // Left side: Title
    title_bar_ui.add_space(12.0);
    let title_text = egui::RichText::new(props.title)
        .size(13.0)
        .color(title_bar_text_color(props.dark_mode));
    title_bar_ui.label(title_text);

    // Right side: Window controls
    title_bar_ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.add_space(8.0);
        render_window_controls(ui, props.dark_mode, events);
    });

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

/// Render Linux window control buttons (minimize, maximize, close)
/// Similar to Windows but with slightly different styling
fn render_window_controls(ui: &mut egui::Ui, dark_mode: bool, events: &mut Vec<TitleBarEvent>) {
    let button_size = egui::vec2(BUTTON_SIZE, BUTTON_SIZE);

    let normal_bg = egui::Color32::TRANSPARENT;
    let hover_bg = if dark_mode {
        egui::Color32::from_rgba_premultiplied(255, 255, 255, 25)
    } else {
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 25)
    };
    let close_hover_bg = egui::Color32::from_rgb(0xf4, 0x43, 0x36); // Material red

    let icon_color = title_bar_text_color(dark_mode);
    let close_icon_hover_color = egui::Color32::WHITE;

    // Close button (red on hover)
    let close_rect = egui::Rect::from_min_size(ui.cursor().min, button_size);
    let close_response = ui.allocate_rect(close_rect, Sense::click());

    let close_bg = if close_response.hovered() {
        close_hover_bg
    } else {
        normal_bg
    };

    let close_icon_color = if close_response.hovered() {
        close_icon_hover_color
    } else {
        icon_color
    };

    ui.painter().rect_filled(close_rect, 4.0, close_bg);

    // Draw X icon using Phosphor
    ui.painter().text(
        close_rect.center(),
        egui::Align2::CENTER_CENTER,
        egui_phosphor::regular::X,
        egui::FontId::proportional(12.0),
        close_icon_color,
    );

    if close_response.clicked() {
        events.push(TitleBarEvent::Close);
    }

    ui.add_space(BUTTON_SPACING);

    // Maximize/Restore button
    let is_maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    let maximize_rect = egui::Rect::from_min_size(ui.cursor().min, button_size);
    let maximize_response = ui.allocate_rect(maximize_rect, Sense::click());

    let maximize_bg = if maximize_response.hovered() {
        hover_bg
    } else {
        normal_bg
    };

    ui.painter().rect_filled(maximize_rect, 4.0, maximize_bg);

    // Draw maximize/restore icon
    let maximize_icon = if is_maximized {
        egui_phosphor::regular::COPY // Restore icon
    } else {
        egui_phosphor::regular::SQUARE // Maximize icon
    };

    ui.painter().text(
        maximize_rect.center(),
        egui::Align2::CENTER_CENTER,
        maximize_icon,
        egui::FontId::proportional(12.0),
        icon_color,
    );

    if maximize_response.clicked() {
        events.push(TitleBarEvent::Maximize);
    }

    ui.add_space(BUTTON_SPACING);

    // Minimize button
    let minimize_rect = egui::Rect::from_min_size(ui.cursor().min, button_size);
    let minimize_response = ui.allocate_rect(minimize_rect, Sense::click());

    let minimize_bg = if minimize_response.hovered() {
        hover_bg
    } else {
        normal_bg
    };

    ui.painter().rect_filled(minimize_rect, 4.0, minimize_bg);

    // Draw minimize icon
    ui.painter().text(
        minimize_rect.center(),
        egui::Align2::CENTER_CENTER,
        egui_phosphor::regular::MINUS,
        egui::FontId::proportional(12.0),
        icon_color,
    );

    if minimize_response.clicked() {
        events.push(TitleBarEvent::Minimize);
    }
}
