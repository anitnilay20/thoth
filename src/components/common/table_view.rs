use eframe::egui;
use egui_extras::{Column, TableBuilder};

use crate::components::traits::StatelessComponent;
use crate::theme::{Theme, ThemeColors};

type BoxedCellRenderer<'a> = Box<dyn Fn(&mut egui::Ui) + 'a>;

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
    /// Column header labels.
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

/// A reusable, horizontally-scrollable, virtually-scrolled table component
/// built on `egui_extras::TableBuilder`. `build_row` is called only for the
/// rows currently visible in the viewport.
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
        let text_height = ui.text_style_height(&egui::TextStyle::Body);
        let header_padding = 10.0;
        let cell_padding = 4.0;
        let row_height = text_height + cell_padding * 2.0;
        let header_height = text_height + header_padding * 2.0;

        let mut clicked_row: Option<usize> = None;
        let mut build_row = props.build_row;

        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(min_col_width * num_cols as f32);

                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .columns(
                        Column::initial(min_col_width).clip(true).resizable(true),
                        num_cols,
                    )
                    .header(header_height, |mut header_row| {
                        for h in props.headers {
                            header_row.col(|ui| {
                                ui.add_space(header_padding);
                                let r = ui.heading(h);
                                let shadow_rect = egui::Rect::from_min_size(
                                    egui::pos2(r.rect.left(), r.rect.bottom()),
                                    egui::vec2(ui.available_width() + min_col_width, 3.0),
                                );
                                ui.painter().rect_filled(
                                    shadow_rect,
                                    0.0,
                                    colors.crust.linear_multiply(0.6),
                                );
                                ui.add_space(header_padding);
                            });
                        }
                    })
                    .body(|body| {
                        // `body.rows` only calls the closure for visible rows —
                        // this is where virtual scrolling actually happens.
                        body.rows(row_height, props.row_count, |mut row| {
                            let cells = build_row(row.index());
                            let mut row_clicked = false;
                            for cell in &cells {
                                let (_, response) = row.col(|ui| {
                                    ui.add_space(cell_padding);
                                    if let Some(custom) = &cell.custom {
                                        custom(ui);
                                    } else {
                                        ui.label(cell.text.unwrap_or(""));
                                    }
                                    ui.add_space(cell_padding);
                                });
                                if response.clicked() {
                                    row_clicked = true;
                                }
                            }
                            if row_clicked {
                                clicked_row = Some(row.index());
                            }
                        });
                    });
            });

        TableViewOutput { clicked_row }
    }
}
