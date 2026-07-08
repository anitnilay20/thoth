use egui::{Color32, Stroke};
use egui_extras::{Column, TableBuilder};

use crate::render_node::UiEvent;
use crate::theme::ThemeColors;

use super::TableView;

const HEADER_H: f32 = 28.0;
const ROW_H: f32 = 30.0;
const NUM_COL_W: f32 = 44.0;
const CELL_PAD: i8 = 10;

impl TableView {
    /// Render the grid, drawing each cell node and collecting their events.
    /// Returns the index of the row clicked this frame, if any.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) -> Option<usize> {
        let colors = ThemeColors::from_ctx(ui.ctx());

        let headers = self.headers.clone();
        let num_cols = headers.len().max(1);
        let min_col_width = self.min_col_width.unwrap_or(150.0);
        // Per-column right-alignment from the (optional) SQL types.
        let right_aligned: Vec<bool> = (0..num_cols)
            .map(|i| self.column_types.get(i).is_some_and(|t| t.right_aligned()))
            .collect();
        // Render cells from an owned copy so the egui_extras closures don't
        // borrow `self`; restore afterwards so cell state persists.
        let mut rows = std::mem::take(&mut self.rows);

        let grid = colors.surface;
        let header_border = colors.surface_raised;
        let header_bg = colors.bg_panel;
        let num_fg = colors.fg_muted;
        let header_fg = colors.fg;

        let mut clicked_row: Option<usize> = None;

        ui.set_min_width(ui.available_width());

        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
                ui.style_mut().spacing.item_spacing.x = 0.0;
                TableBuilder::new(ui)
                    .striped(true)
                    .sense(egui::Sense::click())
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::exact(NUM_COL_W))
                    .columns(
                        Column::auto_with_initial_suggestion(min_col_width)
                            .clip(true)
                            .resizable(true),
                        num_cols,
                    )
                    .header(HEADER_H, |mut header_row| {
                        header_row.col(|ui| {
                            let rect = ui.max_rect();
                            ui.painter().rect_filled(rect, 0.0, header_bg);
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "#",
                                egui::FontId::monospace(10.0),
                                num_fg,
                            );
                            paint_cell_borders(ui, grid, header_border);
                        });
                        for h in &headers {
                            header_row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, header_bg);
                                let (name, ty) = h.split_once("  ·  ").unwrap_or((h.as_str(), ""));
                                let r = egui::Frame::NONE
                                    .inner_margin(egui::Margin::symmetric(CELL_PAD, 0))
                                    .show(ui, |ui| {
                                        ui.style_mut().wrap_mode =
                                            Some(egui::TextWrapMode::Truncate);
                                        let r = ui.label(
                                            egui::RichText::new(name)
                                                .size(11.0)
                                                .strong()
                                                .color(header_fg),
                                        );
                                        if !ty.is_empty() {
                                            ui.add_space(4.0);
                                            ui.label(
                                                egui::RichText::new(ty)
                                                    .size(9.0)
                                                    .monospace()
                                                    .color(num_fg),
                                            );
                                        }
                                        r
                                    })
                                    .inner;
                                if r.hovered() {
                                    r.show_tooltip_text(h);
                                }
                                paint_cell_borders(ui, grid, header_border);
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(ROW_H, rows.len(), |mut row| {
                            let idx = row.index();

                            let mut row_clicked = false;
                            let (_, number_resp) = row.col(|ui| {
                                let rect = ui.max_rect();
                                ui.painter().text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    (idx + 1).to_string(),
                                    egui::FontId::monospace(10.0),
                                    num_fg,
                                );
                                paint_cell_borders(ui, grid, grid);
                            });
                            if number_resp.clicked() {
                                row_clicked = true;
                            }

                            for col in 0..num_cols {
                                let align_right = right_aligned.get(col).copied().unwrap_or(false);
                                let (_, response) = row.col(|ui| {
                                    egui::Frame::NONE
                                        .inner_margin(egui::Margin::symmetric(CELL_PAD, 0))
                                        .show(ui, |ui| {
                                            ui.style_mut().wrap_mode =
                                                Some(egui::TextWrapMode::Truncate);
                                            ui.style_mut().text_styles.insert(
                                                egui::TextStyle::Body,
                                                egui::FontId::proportional(12.0),
                                            );
                                            let layout = if align_right {
                                                egui::Layout::right_to_left(egui::Align::Center)
                                            } else {
                                                egui::Layout::left_to_right(egui::Align::Center)
                                            };
                                            ui.with_layout(layout, |ui| {
                                                if let Some(cell) =
                                                    rows.get_mut(idx).and_then(|r| r.get_mut(col))
                                                {
                                                    cell.show(ui, events);
                                                }
                                            });
                                        });
                                    paint_cell_borders(ui, grid, grid);
                                });
                                if response.clicked() {
                                    row_clicked = true;
                                }
                            }
                            if row_clicked {
                                clicked_row = Some(idx);
                            }
                        });
                    });
            });

        self.rows = rows;
        clicked_row
    }

    /// Render a grid whose rows are produced lazily, one [`RenderNode`] cell per
    /// column. `build_row(idx)` is invoked only for rows currently visible in
    /// the viewport (virtual scrolling), so huge datasets stay cheap. Returns
    /// the row clicked this frame, if any.
    ///
    /// [`RenderNode`]: crate::render_node::RenderNode
    pub fn show_rows(
        ui: &mut egui::Ui,
        headers: &[String],
        row_count: usize,
        min_col_width: Option<f32>,
        events: &mut Vec<UiEvent>,
        mut build_row: impl FnMut(usize) -> Vec<crate::render_node::RenderNode>,
    ) -> Option<usize> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let num_cols = headers.len().max(1);
        let min_col_width = min_col_width.unwrap_or(150.0);

        let grid = colors.surface;
        let header_border = colors.surface_raised;
        let header_bg = colors.bg_panel;
        let num_fg = colors.fg_muted;
        let header_fg = colors.fg;

        let mut clicked_row: Option<usize> = None;

        ui.set_min_width(ui.available_width());

        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
                ui.style_mut().spacing.item_spacing.x = 0.0;
                TableBuilder::new(ui)
                    .striped(true)
                    .sense(egui::Sense::click())
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::exact(NUM_COL_W))
                    .columns(
                        Column::auto_with_initial_suggestion(min_col_width)
                            .clip(true)
                            .resizable(true),
                        num_cols,
                    )
                    .header(HEADER_H, |mut header_row| {
                        header_row.col(|ui| {
                            let rect = ui.max_rect();
                            ui.painter().rect_filled(rect, 0.0, header_bg);
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "#",
                                egui::FontId::monospace(10.0),
                                num_fg,
                            );
                            paint_cell_borders(ui, grid, header_border);
                        });
                        for h in headers {
                            header_row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, header_bg);
                                let (name, ty) = h.split_once("  ·  ").unwrap_or((h.as_str(), ""));
                                let r = egui::Frame::NONE
                                    .inner_margin(egui::Margin::symmetric(CELL_PAD, 0))
                                    .show(ui, |ui| {
                                        ui.style_mut().wrap_mode =
                                            Some(egui::TextWrapMode::Truncate);
                                        let r = ui.label(
                                            egui::RichText::new(name)
                                                .size(11.0)
                                                .strong()
                                                .color(header_fg),
                                        );
                                        if !ty.is_empty() {
                                            ui.add_space(4.0);
                                            ui.label(
                                                egui::RichText::new(ty)
                                                    .size(9.0)
                                                    .monospace()
                                                    .color(num_fg),
                                            );
                                        }
                                        r
                                    })
                                    .inner;
                                if r.hovered() {
                                    r.show_tooltip_text(h);
                                }
                                paint_cell_borders(ui, grid, header_border);
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(ROW_H, row_count, |mut row| {
                            let idx = row.index();
                            let mut cells = build_row(idx);
                            cells.truncate(num_cols);
                            while cells.len() < num_cols {
                                cells.push(crate::render_node::RenderNode::text(""));
                            }

                            let mut row_clicked = false;
                            let (_, number_resp) = row.col(|ui| {
                                let rect = ui.max_rect();
                                ui.painter().text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    (idx + 1).to_string(),
                                    egui::FontId::monospace(10.0),
                                    num_fg,
                                );
                                paint_cell_borders(ui, grid, grid);
                            });
                            if number_resp.clicked() {
                                row_clicked = true;
                            }

                            for cell in &mut cells {
                                let (_, response) = row.col(|ui| {
                                    egui::Frame::NONE
                                        .inner_margin(egui::Margin::symmetric(CELL_PAD, 0))
                                        .show(ui, |ui| {
                                            ui.style_mut().wrap_mode =
                                                Some(egui::TextWrapMode::Truncate);
                                            ui.style_mut().text_styles.insert(
                                                egui::TextStyle::Body,
                                                egui::FontId::proportional(12.0),
                                            );
                                            cell.show(ui, events);
                                        });
                                    paint_cell_borders(ui, grid, grid);
                                });
                                if response.clicked() {
                                    row_clicked = true;
                                }
                            }
                            if row_clicked {
                                clicked_row = Some(idx);
                            }
                        });
                    });
            });

        clicked_row
    }
}

/// Paint a cell's right + bottom grid lines.
fn paint_cell_borders(ui: &egui::Ui, right: Color32, bottom: Color32) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    painter.vline(rect.right() - 0.5, rect.y_range(), Stroke::new(1.0, right));
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, bottom),
    );
}
