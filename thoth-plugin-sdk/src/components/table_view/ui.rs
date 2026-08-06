use egui::{Color32, Stroke};
use egui_extras::{Column, TableBuilder};

use crate::render_node::UiEvent;
use crate::theme::{FONT_CAPTION, RADIUS_PANEL, ThemeColors, edge_stroke, with_alpha};

use super::TableView;

/// Sticky header height — design `.tv thead th{height:28px}`.
const HEADER_H: f32 = 28.0;
/// Body row height — design `.tv tbody td{height:30px}`.
const ROW_H: f32 = 30.0;
/// Width of the `#` gutter — design `.tv th.num,.tv td.num{width:44px}`.
const NUM_COL_W: f32 = 44.0;
/// Horizontal cell padding — design `padding:0 10px`.
const CELL_PAD: i8 = 10;
/// Cell text size — design `.tv table{font-size:12px}`.
const CELL_FONT: f32 = 12.0;
/// `#` gutter text size — design `.tv td.num{font-size:10px}` (monospace).
const NUM_FONT: f32 = 10.0;
/// Column type suffix size — design `.tv thead th .ty{font-size:9px}` (monospace).
const TYPE_FONT: f32 = 9.0;
/// Gap before the type suffix — design `.ty{margin-left:5px}`.
const TYPE_GAP: f32 = 5.0;
/// Zebra wash — design `tbody tr:nth-child(even){background:text 3%}`.
const ZEBRA_ALPHA: u8 = 8;

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

        // A right-click "Copy row / Copy column" is recorded here during
        // rendering and resolved to clipboard text once, after the grid is
        // drawn — so we never build a text matrix every frame just in case.
        let copy_action = std::cell::Cell::new(None::<CopyAction>);

        let grid = colors.surface;

        let mut clicked_row: Option<usize> = None;

        container(ui, self.framed, &colors, |ui| {
            ui.set_min_width(ui.available_width());

            egui::ScrollArea::horizontal()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.style_mut().visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
                    ui.style_mut().spacing.item_spacing.x = 0.0;
                    // Zebra wash, painted *under* the hover/selection fills by
                    // `egui_extras` (design `tbody tr:nth-child(even)`).
                    ui.style_mut().visuals.faint_bg_color = with_alpha(colors.fg, ZEBRA_ALPHA);
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
                        .header(HEADER_H, |header_row| {
                            paint_header_row(header_row, &headers, &colors);
                        })
                        .body(|body| {
                            body.rows(ROW_H, rows.len(), |mut row| {
                                let idx = row.index();

                                let mut row_clicked = false;
                                let (_, number_resp) = row.col(|ui| {
                                    paint_row_number(ui, &colors, &(idx + 1).to_string());
                                    paint_cell_borders(ui, grid, grid);
                                });
                                // The # gutter paints its text (no widget), so its
                                // cell response catches the right-click directly.
                                number_resp.context_menu(|ui| {
                                    copy_menu(ui, &copy_action, idx, None);
                                });
                                if number_resp.clicked() {
                                    row_clicked = true;
                                }

                                for col in 0..num_cols {
                                    let align_right =
                                        right_aligned.get(col).copied().unwrap_or(false);
                                    // Interactive JSON-tree cells handle their own clicks,
                                    // so we don't overlay a right-click target on them.
                                    let is_tree = matches!(
                                        rows.get(idx).and_then(|r| r.get(col)),
                                        Some(crate::render_node::RenderNode::JsonTree(_))
                                    );
                                    let (_, response) = row.col(|ui| {
                                        cell_frame(ui, align_right, |ui| {
                                            if let Some(cell) =
                                                rows.get_mut(idx).and_then(|r| r.get_mut(col))
                                            {
                                                cell.show(ui, events);
                                            }
                                        });
                                        paint_cell_borders(ui, grid, grid);
                                        // The cell's text widget senses only hover but
                                        // still swallows the right-click before the cell
                                        // response sees it, so overlay a full-cell click
                                        // target on top to catch it. Skipped for JSON-tree
                                        // cells, which need their own clicks (their blank
                                        // area is still covered by the outer menu below).
                                        if !is_tree {
                                            let menu_resp = ui.interact(
                                                ui.max_rect(),
                                                ui.id().with("cell-copy-menu"),
                                                egui::Sense::click(),
                                            );
                                            menu_resp.context_menu(|ui| {
                                                copy_menu(ui, &copy_action, idx, Some(col));
                                            });
                                        }
                                    });
                                    // JSON-tree cells have no overlay; let the cell
                                    // response catch right-clicks on their blank area.
                                    if is_tree {
                                        response.context_menu(|ui| {
                                            copy_menu(ui, &copy_action, idx, Some(col));
                                        });
                                    }
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
        });

        // Resolve a requested copy to clipboard text, now that the grid is drawn
        // and `rows` is free to read. Row → cells tab-separated; column → the
        // whole column newline-separated, header first.
        if let Some(action) = copy_action.get() {
            let text = match action {
                CopyAction::Cell(r, c) => rows
                    .get(r)
                    .and_then(|row| row.get(c))
                    .map(node_text)
                    .unwrap_or_default(),
                CopyAction::Row(r) => rows
                    .get(r)
                    .map(|row| row.iter().map(node_text).collect::<Vec<_>>().join("\t"))
                    .unwrap_or_default(),
                CopyAction::Column(c) => {
                    let mut lines: Vec<String> = Vec::with_capacity(rows.len() + 1);
                    if let Some(h) = headers.get(c) {
                        lines.push(
                            h.split_once("  ·  ")
                                .map_or(h.as_str(), |(n, _)| n)
                                .to_string(),
                        );
                    }
                    lines.extend(
                        rows.iter()
                            .map(|row| row.get(c).map(node_text).unwrap_or_default()),
                    );
                    lines.join("\n")
                }
            };
            ui.ctx().copy_text(text);
        }

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

        let mut clicked_row: Option<usize> = None;

        container(ui, true, &colors, |ui| {
            ui.set_min_width(ui.available_width());

            egui::ScrollArea::horizontal()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.style_mut().visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
                    ui.style_mut().spacing.item_spacing.x = 0.0;
                    ui.style_mut().visuals.faint_bg_color = with_alpha(colors.fg, ZEBRA_ALPHA);
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
                        .header(HEADER_H, |header_row| {
                            paint_header_row(header_row, headers, &colors);
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
                                    paint_row_number(ui, &colors, &(idx + 1).to_string());
                                    paint_cell_borders(ui, grid, grid);
                                });
                                if number_resp.clicked() {
                                    row_clicked = true;
                                }

                                for cell in &mut cells {
                                    let (_, response) = row.col(|ui| {
                                        cell_frame(ui, false, |ui| {
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
        });

        clicked_row
    }
}

/// Best-effort plain text of a cell node, for clipboard copy. Covers the node
/// kinds `typed_cell` produces (text, code, badge, and colored wrappers);
/// anything else contributes nothing.
fn node_text(node: &crate::render_node::RenderNode) -> String {
    use crate::render_node::RenderNode as N;
    match node {
        N::Text(t) => t.text.clone(),
        N::Code(c) => c.value.clone(),
        N::Badge(b) => b.label.clone(),
        N::Colored(c) => node_text(&c.child),
        _ => String::new(),
    }
}

/// A copy request recorded from the right-click menu during rendering and
/// resolved to clipboard text after the grid is drawn.
#[derive(Clone, Copy)]
enum CopyAction {
    Cell(usize, usize),
    Row(usize),
    Column(usize),
}

/// Right-click "Copy cell / Copy row / Copy column" menu for a cell. Records the
/// request in `action` (resolved to text post-render); doesn't touch the
/// clipboard itself. `col` is `None` for the row-number gutter, which offers
/// "Copy row" only.
fn copy_menu(
    ui: &mut egui::Ui,
    action: &std::cell::Cell<Option<CopyAction>>,
    row: usize,
    col: Option<usize>,
) {
    // Context-menu text is 2pt smaller than the default.
    for style in [egui::TextStyle::Button, egui::TextStyle::Body] {
        if let Some(font) = ui.style_mut().text_styles.get_mut(&style) {
            font.size = (font.size - 2.0).max(1.0);
        }
    }
    if let Some(col) = col
        && ui.button("Copy cell").clicked()
    {
        action.set(Some(CopyAction::Cell(row, col)));
        ui.close();
    }
    if ui.button("Copy row").clicked() {
        action.set(Some(CopyAction::Row(row)));
        ui.close();
    }
    if let Some(col) = col
        && ui.button("Copy column").clicked()
    {
        action.set(Some(CopyAction::Column(col)));
        ui.close();
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

/// Draw the grid inside the design's outer container (`.tv`): a `bg` fill, a
/// hairline [`edge_stroke`], and [`RADIUS_PANEL`] corners, with the rows clipped
/// to it. `framed` is false when the grid is nested in a container that already
/// owns those corners — [`DataView`], where the design draws the grid flush and
/// border-less.
///
/// [`DataView`]: crate::components::DataView
fn container<R>(
    ui: &mut egui::Ui,
    framed: bool,
    colors: &ThemeColors,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    if !framed {
        return content(ui);
    }
    let inner = egui::Frame::NONE
        .fill(colors.bg)
        .corner_radius(RADIUS_PANEL)
        .show(ui, |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
            content(ui)
        });
    let rect = inner.response.rect;
    // Rows paint square corners over the container's, so mask the corners back
    // to the fill and lay the edge over the topmost row divider.
    paint_corner_mask(ui.painter(), rect, RADIUS_PANEL, colors.bg);
    ui.painter().rect_stroke(
        rect,
        RADIUS_PANEL,
        edge_stroke(colors),
        egui::StrokeKind::Inside,
    );
    inner.inner
}

/// Mask the four corners of `rect` back to `fill`, so square-cornered content
/// doesn't overrun a rounded container. egui has no rounded clip rect, so each
/// corner's outside-the-arc wedge is filled as a triangle fan — one mesh rather
/// than four polygons, which would leave anti-aliasing seams.
pub(crate) fn paint_corner_mask(
    painter: &egui::Painter,
    rect: egui::Rect,
    radius: f32,
    fill: Color32,
) {
    use std::f32::consts::FRAC_PI_2;

    /// Arc segments per corner — a 10px radius needs no more to read as round.
    const SEGMENTS: u32 = 6;

    // (square corner, arc centre, angle of the arc's first point). Angles run
    // clockwise in egui's y-down space, each corner sweeping a quarter turn.
    let corners = [
        (
            rect.left_top(),
            egui::pos2(rect.left() + radius, rect.top() + radius),
            2.0 * FRAC_PI_2,
        ),
        (
            rect.right_top(),
            egui::pos2(rect.right() - radius, rect.top() + radius),
            3.0 * FRAC_PI_2,
        ),
        (
            rect.right_bottom(),
            egui::pos2(rect.right() - radius, rect.bottom() - radius),
            0.0,
        ),
        (
            rect.left_bottom(),
            egui::pos2(rect.left() + radius, rect.bottom() - radius),
            FRAC_PI_2,
        ),
    ];
    let mut mesh = egui::Mesh::default();
    for (corner, centre, start) in corners {
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(corner, fill);
        for i in 0..=SEGMENTS {
            let angle = start + FRAC_PI_2 * i as f32 / SEGMENTS as f32;
            mesh.colored_vertex(centre + radius * egui::vec2(angle.cos(), angle.sin()), fill);
        }
        for i in 0..SEGMENTS {
            mesh.add_triangle(base, base + 1 + i, base + 2 + i);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Paint the sticky header — the `#` gutter plus one cell per column (design
/// `.tv thead th`: mantle fill, a `surface` right divider and a brighter
/// `surface1` bottom divider).
fn paint_header_row(
    mut header_row: egui_extras::TableRow<'_, '_>,
    headers: &[String],
    colors: &ThemeColors,
) {
    header_row.col(|ui| {
        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, colors.bg_panel);
        paint_row_number(ui, colors, "#");
        paint_cell_borders(ui, colors.surface, colors.surface_raised);
    });
    for h in headers {
        let (_, resp) = header_row.col(|ui| {
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, colors.bg_panel);
            paint_header_label(ui, colors, h);
            paint_cell_borders(ui, colors.surface, colors.surface_raised);
        });
        let _ = crate::theme::hover_text(resp, h.as_str());
    }
}

/// Paint one header cell's text: the column name left-aligned inside the 10px
/// padding box at semibold 11px, then the optional `"name  ·  type"` suffix as a
/// small muted mono annotation 5px further right.
fn paint_header_label(ui: &egui::Ui, colors: &ThemeColors, label: &str) {
    let rect = ui.max_rect();
    let pad = f32::from(CELL_PAD);
    let (name, ty) = label.split_once("  ·  ").unwrap_or((label, ""));

    let mut x = rect.left() + pad;
    // Design `thead th{font-weight:600}` — a real semibold face. (The previous
    // `RichText::strong()` here was a no-op, because `strong` only recolours and
    // an explicit colour was already set.)
    let name_galley = layout_line(
        ui.painter(),
        name,
        crate::theme::semibold_font_id(ui.ctx(), FONT_CAPTION),
        colors.fg,
        (rect.right() - pad - x).max(0.0),
    );
    let pos = egui::pos2(x, rect.center().y - name_galley.size().y / 2.0);
    x += name_galley.size().x + TYPE_GAP;
    ui.painter().galley(pos, name_galley, colors.fg);

    if !ty.is_empty() {
        let galley = layout_line(
            ui.painter(),
            ty,
            egui::FontId::monospace(TYPE_FONT),
            colors.fg_muted,
            (rect.right() - pad - x).max(0.0),
        );
        let pos = egui::pos2(x, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(pos, galley, colors.fg_muted);
    }
}

/// Paint a `#` gutter value: right-aligned muted mono, 10px off the right edge
/// (design `.tv th.num,.tv td.num`).
fn paint_row_number(ui: &egui::Ui, colors: &ThemeColors, text: &str) {
    let rect = ui.max_rect();
    ui.painter().text(
        egui::pos2(rect.right() - f32::from(CELL_PAD), rect.center().y),
        egui::Align2::RIGHT_CENTER,
        text,
        egui::FontId::monospace(NUM_FONT),
        colors.fg_muted,
    );
}

/// Run `content` inside a body cell's padding box: 10px horizontal padding, 12px
/// text, no wrapping, and right-to-left flow for numeric/temporal columns.
fn cell_frame(ui: &mut egui::Ui, align_right: bool, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(CELL_PAD, 0))
        .show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(CELL_FONT));
            let layout = if align_right {
                egui::Layout::right_to_left(egui::Align::Center)
            } else {
                egui::Layout::left_to_right(egui::Align::Center)
            };
            ui.with_layout(layout, content);
        });
}

/// Lay out one line of text, ellipsised if it would exceed `max_w` (headers
/// never wrap — design `white-space:nowrap`).
fn layout_line(
    painter: &egui::Painter,
    text: &str,
    font_id: egui::FontId,
    color: Color32,
    max_w: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat {
            font_id,
            color,
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_width: max_w,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    painter.layout_job(job)
}
