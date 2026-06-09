use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::components::traits::StatelessComponent;
use crate::theme::{Theme, ThemeColors};

type BoxedCellRenderer<'a> = Box<dyn Fn(&mut egui::Ui) + 'a>;

#[derive(Default)]
pub struct TableCell<'a> {
    /// Plain text content. Ignored if `custom` is set.
    pub text: Option<&'a str>,
    /// Custom renderer for the cell — overrides `text` when provided.
    pub custom: Option<BoxedCellRenderer<'a>>,
}

impl<'a> TableCell<'a> {
    pub fn text(text: &'a str) -> Self {
        Self {
            text: Some(text),
            custom: None,
        }
    }

    pub fn custom(f: impl Fn(&mut egui::Ui) + 'a) -> Self {
        Self {
            text: None,
            custom: Some(Box::new(f)),
        }
    }
}

pub struct TableViewProps<'a> {
    /// Column header labels. A label of the form `"name  ·  type"` renders the
    /// name in the header weight and the type as a small muted mono suffix.
    pub headers: &'a [String],
    /// Total number of rows. Only the visible subset will be rendered.
    pub row_count: usize,
    /// Called once per *visible* row — return exactly `headers.len()` cells.
    /// Owns strings/nodes for that row; called lazily by the virtual scroller.
    pub build_row: Box<dyn FnMut(usize) -> Vec<TableCell<'a>> + 'a>,
    /// Minimum width per column in logical pixels. Defaults to 150.
    pub min_col_width: Option<f32>,
}

/// Output of a `TableView` render pass.
pub struct TableViewOutput {
    /// Index of the row that was clicked, if any.
    pub clicked_row: Option<usize>,
}

// Grid metrics — tuned to the design handoff's results grid.
const HEADER_H: f32 = 28.0;
const ROW_H: f32 = 30.0;
const NUM_COL_W: f32 = 44.0;
const CELL_PAD_X: f32 = 10.0;

/// A reusable, horizontally-scrollable, virtually-scrolled data grid built on
/// `egui_extras::TableBuilder`, styled after the design handoff: a sticky `#`
/// row-number column, a compact header (name + muted mono type), `surface`
/// grid lines, zebra rows, and a stronger header underline. `build_row` is
/// called only for the rows currently visible in the viewport.
pub struct TableView;

impl StatelessComponent for TableView {
    type Props<'a> = TableViewProps<'a>;
    type Output = TableViewOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| Theme::default().colors())
        });

        let num_cols = props.headers.len().max(1);
        let min_col_width = props.min_col_width.unwrap_or(150.0);

        // Semantic palette (maps the design tokens onto the theme).
        let grid = colors.surface; // --surface0: cell grid lines
        let header_border = colors.surface_raised; // --surface1: header underline
        let header_bg = colors.bg_panel; // --mantle: header strip
        let num_fg = colors.fg_muted; // row numbers + type suffix
        let header_fg = colors.fg;

        let mut clicked_row: Option<usize> = None;
        let mut build_row = props.build_row;

        ui.set_min_width(ui.available_width());

        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                TableBuilder::new(ui)
                    .striped(true)
                    .sense(egui::Sense::click())
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(Column::exact(NUM_COL_W)) // row-number gutter
                    .columns(
                        Column::auto_with_initial_suggestion(min_col_width)
                            .clip(true)
                            .resizable(true),
                        num_cols,
                    )
                    .header(HEADER_H, |mut header_row| {
                        // `#` gutter header.
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
                        for h in props.headers {
                            header_row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, header_bg);
                                ui.add_space(CELL_PAD_X);
                                let (name, ty) = h.split_once("  ·  ").unwrap_or((h.as_str(), ""));
                                let r = ui.label(
                                    egui::RichText::new(name)
                                        .size(11.0)
                                        .strong()
                                        .color(header_fg),
                                );
                                if !ty.is_empty() {
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new(ty).size(9.0).monospace().color(num_fg),
                                    );
                                }
                                if r.hovered() {
                                    r.show_tooltip_text(h);
                                }
                                paint_cell_borders(ui, grid, header_border);
                            });
                        }
                    })
                    .body(|body| {
                        // `body.rows` only invokes the closure for visible rows —
                        // this is where virtual scrolling actually happens.
                        body.rows(ROW_H, props.row_count, |mut row| {
                            let idx = row.index();
                            let mut cells = build_row(idx);
                            debug_assert_eq!(
                                cells.len(),
                                num_cols,
                                "build_row returned {} cells but expected {} (headers.len())",
                                cells.len(),
                                num_cols
                            );
                            // Truncate extra cells or pad missing ones so columns
                            // never mis-align in release builds.
                            cells.truncate(num_cols);
                            while cells.len() < num_cols {
                                cells.push(TableCell::default());
                            }

                            // `#` gutter cell.
                            row.col(|ui| {
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

                            let mut row_clicked = false;
                            for cell in &cells {
                                let (_, response) = row.col(|ui| {
                                    ui.add_space(CELL_PAD_X);
                                    if let Some(custom) = &cell.custom {
                                        custom(ui);
                                    } else {
                                        ui.label(cell.text.unwrap_or(""));
                                    }
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

        TableViewOutput { clicked_row }
    }
}

/// Paint a cell's right + bottom grid lines (inset slightly so they aren't
/// clipped at the cell boundary). `right`/`bottom` let the header use a stronger
/// underline than the vertical lines.
fn paint_cell_borders(ui: &egui::Ui, right: egui::Color32, bottom: egui::Color32) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    painter.vline(
        rect.right() - 0.5,
        rect.y_range(),
        egui::Stroke::new(1.0, right),
    );
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        egui::Stroke::new(1.0, bottom),
    );
}
