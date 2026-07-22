//! A generated chart, living as its own dock tab.
//!
//! Owns a *snapshot* of the source data (columns + string rows) plus the
//! resolved spec, so it keeps rendering after the source tab closes. Cartesian
//! types render with `egui_plot`; radial types and the heatmap use a custom
//! `egui` painter. All colours come from the active theme.

use std::f32::consts::TAU;

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use egui_plot::{Bar, BarChart, Legend, Line, Plot, PlotPoints, Points};

use super::{ChartOptions, ChartSpec, ChartTabAction, ChartType, series_palette};
use crate::app::tab_manager::TabId;
use crate::theme::ThemeColors;

/// Rows kept per chart (bounds memory / render cost for huge sources).
const ROW_CAP: usize = 5000;
/// Numeric columns shown in a heatmap.
const HEATMAP_COL_CAP: usize = 8;

pub struct ChartTab {
    tab_title: String,
    subtitle: String,
    chart_type: ChartType,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    x_col: usize,
    y_cols: Vec<usize>,
    options: ChartOptions,
    /// The producer tab this chart was built from (for Refresh / Edit).
    source_tab: TabId,
    source_label: String,
}

impl ChartTab {
    /// Build a chart tab from a spec and a freshly-resolved data snapshot.
    /// `index` is the per-session chart counter used for the tab label.
    pub fn from_spec(
        spec: &ChartSpec,
        columns: Vec<String>,
        mut rows: Vec<Vec<String>>,
        index: usize,
    ) -> Self {
        rows.truncate(ROW_CAP);
        let mut tab = Self {
            tab_title: format!("{} {index}", spec.chart_type.label()),
            subtitle: String::new(),
            chart_type: spec.chart_type,
            columns,
            rows,
            x_col: spec.x_col,
            y_cols: spec.y_cols.clone(),
            options: spec.options,
            source_tab: spec.source_tab,
            source_label: spec.source_label.clone(),
        };
        tab.rebuild_subtitle();
        tab
    }

    pub fn tab_title(&self) -> String {
        self.tab_title.clone()
    }

    pub fn source_tab(&self) -> TabId {
        self.source_tab
    }

    /// Compact one-line summary for the status bar.
    pub fn status_summary(&self) -> String {
        format!(
            "{} · {} rows · {} series",
            self.chart_type.label(),
            self.rows.len(),
            self.y_cols.len()
        )
    }

    /// Reconstruct the spec, so the studio can re-open this chart for editing.
    pub fn to_spec(&self) -> ChartSpec {
        ChartSpec {
            source_tab: self.source_tab,
            source_label: self.source_label.clone(),
            chart_type: self.chart_type,
            x_col: self.x_col,
            y_cols: self.y_cols.clone(),
            options: self.options,
            edit_target: None,
        }
    }

    /// Replace the data snapshot (Refresh): keep the spec but clamp axis indices
    /// to the new column count and rebuild the subtitle.
    pub fn update_data(&mut self, columns: Vec<String>, mut rows: Vec<Vec<String>>) {
        rows.truncate(ROW_CAP);
        let max_col = columns.len().saturating_sub(1);
        self.x_col = self.x_col.min(max_col);
        for c in &mut self.y_cols {
            *c = (*c).min(max_col);
        }
        self.columns = columns;
        self.rows = rows;
        self.rebuild_subtitle();
    }

    fn rebuild_subtitle(&mut self) {
        self.subtitle = format!(
            "{} · {} rows · {} series",
            self.source_label,
            self.rows.len(),
            self.y_cols.len()
        );
    }

    fn header_title(&self) -> String {
        let name = |c: usize| self.columns.get(c).cloned().unwrap_or_default();
        let y_names = self
            .y_cols
            .iter()
            .map(|&c| name(c))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{}: {} / {}",
            self.chart_type.label(),
            y_names,
            name(self.x_col)
        )
    }

    // ── data helpers ────────────────────────────────────────────────────────

    fn val(&self, row: usize, col: usize) -> Option<f64> {
        self.rows.get(row)?.get(col)?.trim().parse::<f64>().ok()
    }

    fn x_label(&self, row: usize) -> String {
        self.rows
            .get(row)
            .and_then(|r| r.get(self.x_col))
            .cloned()
            .unwrap_or_default()
    }

    /// Indices of columns whose values parse as numbers in most sampled rows.
    fn numeric_columns(&self) -> Vec<usize> {
        let sample = self.rows.len().clamp(1, 32);
        (0..self.columns.len())
            .filter(|&c| {
                let ok = (0..sample).filter(|&r| self.val(r, c).is_some()).count();
                ok * 2 >= sample
            })
            .collect()
    }

    // ── entry point ─────────────────────────────────────────────────────────

    pub fn render(&mut self, ui: &mut egui::Ui, colors: &ThemeColors) -> Option<ChartTabAction> {
        use thoth_plugin_sdk::components::{IconButton, Typography, TypographyVariant};
        let mut action = None;

        // Header: title/subtitle on the left, Edit + Refresh tools on the right.
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.add(
                    Typography::builder()
                        .text(self.header_title())
                        .variant(TypographyVariant::BodyLarge)
                        .build(),
                );
                ui.add(
                    Typography::builder()
                        .text(&self.subtitle)
                        .variant(TypographyVariant::BodyMuted)
                        .build(),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        IconButton::builder()
                            .icon(egui_phosphor::regular::ARROWS_CLOCKWISE)
                            .frame(true)
                            .tooltip("Refresh data from source")
                            .build(),
                    )
                    .clicked()
                {
                    action = Some(ChartTabAction::Refresh);
                }
                if ui
                    .add(
                        IconButton::builder()
                            .icon(egui_phosphor::regular::PENCIL_SIMPLE)
                            .frame(true)
                            .tooltip("Edit chart in Chart Studio")
                            .build(),
                    )
                    .clicked()
                {
                    action = Some(ChartTabAction::Edit);
                }
            });
        });
        ui.add_space(8.0);

        if self.rows.is_empty() {
            self.empty_note(ui, "This dataset has no rows to plot.");
            return action;
        }

        // Chart surface
        egui::Frame::new()
            .fill(colors.surface)
            .stroke(Stroke::new(1.0, colors.surface_raised))
            .corner_radius(8.0)
            .inner_margin(14.0)
            .show(ui, |ui| {
                let size = ui.available_size();
                match self.chart_type {
                    ChartType::Bar
                    | ChartType::HBar
                    | ChartType::Line
                    | ChartType::Area
                    | ChartType::Scatter
                    | ChartType::Histogram => self.render_plot(ui, colors),
                    ChartType::Pie | ChartType::Doughnut => self.render_pie(ui, colors, size),
                    ChartType::PolarArea => self.render_polar(ui, colors, size),
                    ChartType::Radar => self.render_radar(ui, colors, size),
                    ChartType::Heatmap => self.render_heatmap(ui, colors, size),
                }
            });
        action
    }

    // ── cartesian (egui_plot) ────────────────────────────────────────────────

    fn render_plot(&self, ui: &mut egui::Ui, colors: &ThemeColors) {
        let palette = series_palette(colors);
        let categorical = !matches!(self.chart_type, ChartType::Scatter);

        let mut plot = Plot::new("chart_plot")
            .show_grid(self.options.grid)
            .allow_scroll(false);
        if self.options.legend {
            plot = plot.legend(Legend::default());
        }
        // Map categorical x ticks back to their string labels.
        if categorical && !matches!(self.chart_type, ChartType::HBar) {
            let labels: Vec<String> = (0..self.rows.len()).map(|r| self.x_label(r)).collect();
            plot = plot.x_axis_formatter(move |mark, _| tick_label(&labels, mark.value));
        }
        if matches!(self.chart_type, ChartType::HBar) {
            let labels: Vec<String> = (0..self.rows.len()).map(|r| self.x_label(r)).collect();
            plot = plot.y_axis_formatter(move |mark, _| tick_label(&labels, mark.value));
        }

        let resp = plot.show(ui, |plot_ui| match self.chart_type {
            ChartType::Line | ChartType::Area => self.plot_lines(plot_ui, &palette),
            ChartType::Scatter => self.plot_scatter(plot_ui, &palette),
            ChartType::Histogram => self.plot_histogram(plot_ui, &palette),
            ChartType::HBar => self.plot_bars(plot_ui, &palette, true),
            _ => self.plot_bars(plot_ui, &palette, false),
        });

        // Hover: show the full data row for the point/bar under the cursor.
        if let Some(pos) = resp.response.hover_pos()
            && resp.transform.frame().contains(pos)
            && let Some(row) = self.cartesian_hover_row(pos, &resp.transform)
        {
            self.show_row_tooltip(&resp.response, row);
        }
    }

    /// The row under the cursor for a cartesian chart (index-based for
    /// categorical types, nearest point for scatter). `None` for histogram
    /// (bins aggregate rows) or when the cursor isn't near data.
    fn cartesian_hover_row(&self, pos: egui::Pos2, t: &egui_plot::PlotTransform) -> Option<usize> {
        if matches!(self.chart_type, ChartType::Histogram) {
            return None;
        }
        if matches!(self.chart_type, ChartType::Scatter) {
            let mut best: Option<(f32, usize)> = None;
            for r in 0..self.rows.len() {
                let x = self.val(r, self.x_col).unwrap_or(r as f64);
                for &col in &self.y_cols {
                    if let Some(y) = self.val(r, col) {
                        let d = t
                            .position_from_point(&egui_plot::PlotPoint::new(x, y))
                            .distance(pos);
                        if best.is_none_or(|(bd, _)| d < bd) {
                            best = Some((d, r));
                        }
                    }
                }
            }
            return best.filter(|(d, _)| *d <= 24.0).map(|(_, r)| r);
        }
        let pt = t.value_from_position(pos);
        let v = if matches!(self.chart_type, ChartType::HBar) {
            pt.y
        } else {
            pt.x
        };
        let i = v.round();
        (i >= 0.0 && (i as usize) < self.rows.len() && (v - i).abs() <= 0.5).then_some(i as usize)
    }

    /// Show a tooltip at the pointer listing every column of `row`.
    fn show_row_tooltip(&self, area: &egui::Response, row: usize) {
        egui::Tooltip::for_widget(area)
            .at_pointer()
            .show(|ui| self.row_tooltip(ui, row));
    }

    fn row_tooltip(&self, ui: &mut egui::Ui, row: usize) {
        use thoth_plugin_sdk::components::{Typography, TypographyVariant};
        let Some(r) = self.rows.get(row) else {
            return;
        };
        egui::Grid::new("chart_row_tip")
            .num_columns(2)
            .spacing([12.0, 2.0])
            .show(ui, |ui| {
                for (c, name) in self.columns.iter().enumerate() {
                    ui.add(
                        Typography::builder()
                            .text(name)
                            .variant(TypographyVariant::Label)
                            .build(),
                    );
                    ui.add(
                        Typography::builder()
                            .text(r.get(c).cloned().unwrap_or_default())
                            .variant(TypographyVariant::Body)
                            .build(),
                    );
                    ui.end_row();
                }
            });
    }

    fn plot_lines(&self, plot_ui: &mut egui_plot::PlotUi, palette: &[Color32; 8]) {
        let area = matches!(self.chart_type, ChartType::Area);
        for (si, &col) in self.y_cols.iter().enumerate() {
            let pts: Vec<[f64; 2]> = (0..self.rows.len())
                .filter_map(|r| self.val(r, col).map(|y| [r as f64, y]))
                .collect();
            if pts.is_empty() {
                continue;
            }
            let color = palette[si % palette.len()];
            let mut line = Line::new(self.columns[col].clone(), PlotPoints::from(pts))
                .color(color)
                .width(2.0);
            if area {
                line = line.fill(0.0).fill_alpha(0.18);
            }
            plot_ui.line(line);
        }
    }

    fn plot_scatter(&self, plot_ui: &mut egui_plot::PlotUi, palette: &[Color32; 8]) {
        for (si, &col) in self.y_cols.iter().enumerate() {
            let pts: Vec<[f64; 2]> = (0..self.rows.len())
                .filter_map(|r| {
                    let y = self.val(r, col)?;
                    let x = self.val(r, self.x_col).unwrap_or(r as f64);
                    Some([x, y])
                })
                .collect();
            if pts.is_empty() {
                continue;
            }
            plot_ui.points(
                Points::new(self.columns[col].clone(), PlotPoints::from(pts))
                    .color(palette[si % palette.len()])
                    .radius(3.0)
                    .filled(true),
            );
        }
    }

    #[allow(clippy::needless_range_loop)] // r indexes both self.rows (via val) and stack_base
    fn plot_bars(&self, plot_ui: &mut egui_plot::PlotUi, palette: &[Color32; 8], horizontal: bool) {
        let n = self.y_cols.len().max(1);
        let group_w = 0.82;
        let bar_w = group_w / n as f64;
        // Stacking accumulates a base offset per row *across* series.
        let mut stack_base = vec![0.0_f64; self.rows.len()];
        for (si, &col) in self.y_cols.iter().enumerate() {
            let color = palette[si % palette.len()];
            let mut bars = Vec::new();
            for r in 0..self.rows.len() {
                let Some(v) = self.val(r, col) else { continue };
                let (arg, width, base) = if self.options.stacked {
                    (r as f64, group_w, stack_base[r])
                } else {
                    (
                        r as f64 - group_w / 2.0 + bar_w * (si as f64 + 0.5),
                        bar_w * 0.92,
                        0.0,
                    )
                };
                let mut bar = Bar::new(arg, v)
                    .name(&self.columns[col])
                    .fill(color)
                    .width(width)
                    .base_offset(base);
                if horizontal {
                    bar = bar.horizontal();
                }
                bars.push(bar);
                if self.options.stacked {
                    stack_base[r] += v;
                }
            }
            plot_ui.bar_chart(BarChart::new(self.columns[col].clone(), bars).color(color));
        }
    }

    fn plot_histogram(&self, plot_ui: &mut egui_plot::PlotUi, palette: &[Color32; 8]) {
        let col = *self.y_cols.first().unwrap_or(&0);
        let vals: Vec<f64> = (0..self.rows.len())
            .filter_map(|r| self.val(r, col))
            .collect();
        if vals.is_empty() {
            return;
        }
        let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let bins = 8usize;
        let step = if max > min {
            (max - min) / bins as f64
        } else {
            1.0
        };
        let mut counts = vec![0.0_f64; bins];
        for v in vals {
            let mut b = ((v - min) / step).floor() as usize;
            if b >= bins {
                b = bins - 1;
            }
            counts[b] += 1.0;
        }
        let bars: Vec<Bar> = counts
            .iter()
            .enumerate()
            .map(|(i, &c)| {
                Bar::new(min + step * (i as f64 + 0.5), c)
                    .width(step * 0.94)
                    .fill(palette[0])
            })
            .collect();
        plot_ui.bar_chart(
            BarChart::new(format!("{} (count)", self.columns[col]), bars).color(palette[0]),
        );
    }

    // ── radial + heatmap (custom painter) ─────────────────────────────────────

    /// Reserve a plotting square and (optionally) a legend column; returns the
    /// square's centre + radius and paints the legend entries.
    fn radial_frame(
        &self,
        ui: &mut egui::Ui,
        colors: &ThemeColors,
        size: Vec2,
        entries: &[(String, Color32)],
    ) -> (Pos2, f32, egui::Response) {
        let (rect, area) = ui.allocate_exact_size(size, egui::Sense::hover());
        let legend_w = if self.options.legend && !entries.is_empty() {
            (size.x * 0.28).min(160.0)
        } else {
            0.0
        };
        let plot_rect =
            Rect::from_min_size(rect.min, Vec2::new(rect.width() - legend_w, rect.height()));
        let radius = (plot_rect.width().min(plot_rect.height()) * 0.5 - 12.0).max(10.0);
        if legend_w > 0.0 {
            let painter = ui.painter_at(rect);
            let mut y = rect.top() + 6.0;
            for (label, color) in entries {
                let sw = Rect::from_min_size(
                    Pos2::new(rect.right() - legend_w + 4.0, y + 2.0),
                    Vec2::splat(10.0),
                );
                painter.rect_filled(sw, 2.0, *color);
                painter.text(
                    Pos2::new(sw.right() + 6.0, y + 7.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(11.0),
                    colors.fg_muted,
                );
                y += 18.0;
            }
        }
        (plot_rect.center(), radius, area)
    }

    fn render_pie(&self, ui: &mut egui::Ui, colors: &ThemeColors, size: Vec2) {
        let col = *self.y_cols.first().unwrap_or(&0);
        // Keep the source row index so hover can show its full row.
        let slices: Vec<(usize, f64)> = (0..self.rows.len())
            .filter_map(|r| self.val(r, col).filter(|v| *v > 0.0).map(|v| (r, v)))
            .collect();
        let total: f64 = slices.iter().map(|(_, v)| v).sum();
        if total <= 0.0 {
            self.empty_note(ui, "No positive values to plot.");
            return;
        }
        let palette = series_palette(colors);
        let entries: Vec<(String, Color32)> = slices
            .iter()
            .enumerate()
            .map(|(i, (r, _))| (self.x_label(*r), palette[i % palette.len()]))
            .collect();
        let (center, radius, area) = self.radial_frame(ui, colors, size, &entries);
        let inner = if matches!(self.chart_type, ChartType::Doughnut) {
            radius * 0.55
        } else {
            0.0
        };
        let painter = ui.painter();
        let mut a0 = -TAU / 4.0;
        let mut bounds = Vec::with_capacity(slices.len());
        for (i, (r, v)) in slices.iter().enumerate() {
            let a1 = a0 + (*v / total) as f32 * TAU;
            let poly = annular_sector(center, inner, radius, a0, a1);
            painter.add(egui::Shape::convex_polygon(
                poly,
                palette[i % palette.len()],
                Stroke::new(1.5, colors.bg),
            ));
            bounds.push((*r, a0, a1));
            a0 = a1;
        }
        if let Some(pos) = ui.ctx().pointer_hover_pos() {
            let dist = (pos - center).length();
            if dist >= inner
                && dist <= radius
                && let Some(row) = wedge_row(center, pos, &bounds)
            {
                self.show_row_tooltip(&area, row);
            }
        }
    }

    fn render_polar(&self, ui: &mut egui::Ui, colors: &ThemeColors, size: Vec2) {
        let col = *self.y_cols.first().unwrap_or(&0);
        let items: Vec<(usize, f64)> = (0..self.rows.len())
            .filter_map(|r| self.val(r, col).map(|v| (r, v.max(0.0))))
            .collect();
        let max = items.iter().map(|(_, v)| *v).fold(0.0_f64, f64::max);
        if max <= 0.0 {
            self.empty_note(ui, "No positive values to plot.");
            return;
        }
        let palette = series_palette(colors);
        let entries: Vec<(String, Color32)> = items
            .iter()
            .enumerate()
            .map(|(i, (r, _))| (self.x_label(*r), palette[i % palette.len()]))
            .collect();
        let (center, radius, area) = self.radial_frame(ui, colors, size, &entries);
        let painter = ui.painter();
        let n = items.len().max(1);
        let seg = TAU / n as f32;
        let mut a0 = -TAU / 4.0;
        let mut bounds = Vec::with_capacity(items.len());
        for (i, (r, v)) in items.iter().enumerate() {
            let ri = (*v / max) as f32 * radius;
            let poly = annular_sector(center, 0.0, ri, a0, a0 + seg);
            painter.add(egui::Shape::convex_polygon(
                poly,
                with_alpha(palette[i % palette.len()], 0.82),
                Stroke::new(1.0, colors.bg),
            ));
            bounds.push((*r, a0, a0 + seg, ri));
            a0 += seg;
        }
        if let Some(pos) = ui.ctx().pointer_hover_pos() {
            let dist = (pos - center).length();
            if let Some(row) = wedge_row(
                center,
                pos,
                &bounds
                    .iter()
                    .map(|&(r, a0, a1, _)| (r, a0, a1))
                    .collect::<Vec<_>>(),
            ) && bounds.iter().any(|&(r, _, _, ri)| r == row && dist <= ri)
            {
                self.show_row_tooltip(&area, row);
            }
        }
    }

    fn render_radar(&self, ui: &mut egui::Ui, colors: &ThemeColors, size: Vec2) {
        let axes = self.rows.len();
        if axes < 3 {
            self.empty_note(ui, "Radar needs at least 3 rows.");
            return;
        }
        let palette = series_palette(colors);
        let entries: Vec<(String, Color32)> = self
            .y_cols
            .iter()
            .enumerate()
            .map(|(i, &c)| (self.columns[c].clone(), palette[i % palette.len()]))
            .collect();
        let max = self
            .y_cols
            .iter()
            .flat_map(|&c| (0..axes).filter_map(move |r| self.val(r, c)))
            .fold(0.0_f64, f64::max);
        if max <= 0.0 {
            self.empty_note(ui, "No positive values to plot.");
            return;
        }
        let (center, radius, area) = self.radial_frame(ui, colors, size, &entries);
        let painter = ui.painter();
        let angle = |j: usize| -TAU / 4.0 + j as f32 / axes as f32 * TAU;

        // Grid rings + spokes.
        if self.options.grid {
            for ring in 1..=4 {
                let rr = radius * ring as f32 / 4.0;
                let pts: Vec<Pos2> = (0..axes)
                    .map(|j| center + Vec2::angled(angle(j)) * rr)
                    .collect();
                for w in 0..axes {
                    painter.line_segment(
                        [pts[w], pts[(w + 1) % axes]],
                        Stroke::new(1.0, with_alpha(colors.surface_raised, 0.5)),
                    );
                }
            }
            for j in 0..axes {
                painter.line_segment(
                    [center, center + Vec2::angled(angle(j)) * radius],
                    Stroke::new(1.0, with_alpha(colors.surface_raised, 0.5)),
                );
                painter.text(
                    center + Vec2::angled(angle(j)) * (radius + 10.0),
                    egui::Align2::CENTER_CENTER,
                    self.x_label(j),
                    FontId::proportional(9.0),
                    colors.fg_muted,
                );
            }
        }

        for (si, &col) in self.y_cols.iter().enumerate() {
            let color = palette[si % palette.len()];
            let poly: Vec<Pos2> = (0..axes)
                .map(|j| {
                    let v = self.val(j, col).unwrap_or(0.0).max(0.0);
                    center + Vec2::angled(angle(j)) * ((v / max) as f32 * radius)
                })
                .collect();
            painter.add(egui::Shape::convex_polygon(
                poly.clone(),
                with_alpha(color, 0.16),
                Stroke::new(2.0, color),
            ));
        }

        // Hover: the axis (row) nearest the cursor's angle.
        if let Some(pos) = ui.ctx().pointer_hover_pos() {
            let v = pos - center;
            if v.length() <= radius {
                let a = v.y.atan2(v.x);
                let mut best = (f32::MAX, 0usize);
                for j in 0..axes {
                    let d = angle_diff(a, angle(j));
                    if d < best.0 {
                        best = (d, j);
                    }
                }
                self.show_row_tooltip(&area, best.1);
            }
        }
    }

    fn render_heatmap(&self, ui: &mut egui::Ui, colors: &ThemeColors, size: Vec2) {
        let cols: Vec<usize> = self
            .numeric_columns()
            .into_iter()
            .take(HEATMAP_COL_CAP)
            .collect();
        if cols.is_empty() {
            self.empty_note(ui, "No numeric columns for a heatmap.");
            return;
        }
        let (rect, area) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        let pad_left = 70.0;
        let pad_top = 22.0;
        let grid = Rect::from_min_max(
            Pos2::new(rect.left() + pad_left, rect.top() + pad_top),
            rect.max,
        );
        let rows = self.rows.len();
        let cw = grid.width() / cols.len() as f32;
        let rh = grid.height() / rows.max(1) as f32;

        // Per-column min/max for normalisation.
        let ranges: Vec<(f64, f64)> = cols
            .iter()
            .map(|&c| {
                let vals: Vec<f64> = (0..rows).filter_map(|r| self.val(r, c)).collect();
                let mn = vals.iter().cloned().fold(f64::INFINITY, f64::min);
                let mx = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                (mn, mx)
            })
            .collect();

        for (ci, &c) in cols.iter().enumerate() {
            painter.text(
                Pos2::new(
                    grid.left() + cw * (ci as f32 + 0.5),
                    rect.top() + pad_top / 2.0,
                ),
                egui::Align2::CENTER_CENTER,
                &self.columns[c],
                FontId::proportional(10.0),
                colors.fg_muted,
            );
        }
        for r in 0..rows {
            let y = grid.top() + rh * r as f32;
            if rh > 12.0 {
                painter.text(
                    Pos2::new(rect.left() + pad_left - 6.0, y + rh / 2.0),
                    egui::Align2::RIGHT_CENTER,
                    trunc(&self.x_label(r), 9),
                    FontId::proportional(9.0),
                    colors.fg_muted,
                );
            }
            for (ci, &c) in cols.iter().enumerate() {
                let (mn, mx) = ranges[ci];
                let t = match self.val(r, c) {
                    Some(v) if mx > mn => ((v - mn) / (mx - mn)) as f32,
                    Some(_) => 0.5,
                    None => continue,
                };
                let x = grid.left() + cw * ci as f32;
                let cell =
                    Rect::from_min_size(Pos2::new(x + 1.0, y + 1.0), Vec2::new(cw - 2.0, rh - 2.0));
                painter.rect_filled(cell, 2.0, heat_color(t, colors));
                if rh > 18.0
                    && cw > 34.0
                    && let Some(v) = self.val(r, c)
                {
                    painter.text(
                        cell.center(),
                        egui::Align2::CENTER_CENTER,
                        fmt_num(v),
                        FontId::proportional(9.0),
                        if t > 0.55 { colors.bg } else { colors.fg },
                    );
                }
            }
        }

        // Hover: the row (line) under the cursor.
        if let Some(pos) = ui.ctx().pointer_hover_pos()
            && grid.contains(pos)
            && rh > 0.0
        {
            let r = ((pos.y - grid.top()) / rh).floor() as i32;
            if r >= 0 && (r as usize) < rows {
                self.show_row_tooltip(&area, r as usize);
            }
        }
    }

    fn empty_note(&self, ui: &mut egui::Ui, msg: &str) {
        use thoth_plugin_sdk::components::{Typography, TypographyVariant};
        ui.add(
            Typography::builder()
                .text(msg)
                .variant(TypographyVariant::BodyMuted)
                .build(),
        );
    }
}

// ── free helpers ──────────────────────────────────────────────────────────────

/// Label for a categorical axis tick — only integer marks map to a row.
fn tick_label(labels: &[String], value: f64) -> String {
    let i = value.round();
    if (value - i).abs() < 1e-6 && i >= 0.0 && (i as usize) < labels.len() {
        labels[i as usize].clone()
    } else {
        String::new()
    }
}

/// Points of an annular sector (pie wedge when `r_inner == 0`).
fn annular_sector(center: Pos2, r_inner: f32, r_outer: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let segs = (((a1 - a0).abs() / (TAU / 120.0)).ceil() as usize).max(2);
    let mut pts = Vec::with_capacity(segs * 2 + 2);
    for i in 0..=segs {
        let a = a0 + (a1 - a0) * i as f32 / segs as f32;
        pts.push(center + Vec2::angled(a) * r_outer);
    }
    if r_inner <= 0.5 {
        pts.push(center);
    } else {
        for i in (0..=segs).rev() {
            let a = a0 + (a1 - a0) * i as f32 / segs as f32;
            pts.push(center + Vec2::angled(a) * r_inner);
        }
    }
    pts
}

/// Which wedge (row) the cursor is over, by angle. `bounds` is `(row, a0, a1)`
/// with angles measured like `annular_sector` (from `-TAU/4`, increasing).
fn wedge_row(center: Pos2, pos: Pos2, bounds: &[(usize, f32, f32)]) -> Option<usize> {
    let start = -TAU / 4.0;
    let v = pos - center;
    let mut a = v.y.atan2(v.x);
    while a < start {
        a += TAU;
    }
    while a >= start + TAU {
        a -= TAU;
    }
    bounds
        .iter()
        .find(|(_, a0, a1)| a >= *a0 && a < *a1)
        .map(|(r, _, _)| *r)
}

/// Smallest absolute angular distance between two angles (radians).
fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (a - b).abs() % TAU;
    if d > TAU / 2.0 {
        d = TAU - d;
    }
    d
}

fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Low→high heat colour ramp built from theme role tokens.
fn heat_color(t: f32, colors: &ThemeColors) -> Color32 {
    let stops = [
        colors.bg,
        colors.surface_active,
        colors.accent_secondary,
        colors.accent,
        colors.warning,
    ];
    let seg = t.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let lo = seg.floor() as usize;
    let hi = (lo + 1).min(stops.len() - 1);
    lerp_color(stops[lo], stops[hi], seg - lo as f32)
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Compact numeric label (12k, 1.2M, 4.2).
fn fmt_num(v: f64) -> String {
    let a = v.abs();
    if a >= 1e6 {
        format!("{:.1}M", v / 1e6)
    } else if a >= 1e3 {
        format!("{:.0}k", v / 1e3)
    } else if v.fract() != 0.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.0}")
    }
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() > n {
        s.chars().take(n).collect()
    } else {
        s.to_string()
    }
}
