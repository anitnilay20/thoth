//! The Chart Studio **config panel** rendered in the sidebar content area.
//!
//! Holds the current selection (source, type, axes, options) plus the
//! app-injected producer list, resolved column schema, and open-chart list.
//! Emits [`ChartStudioEvent`]s up to the app, which does the data fetching and
//! tab creation.
//!
//! Built entirely from `thoth-plugin-sdk` components (Select, NumberInput,
//! ToggleSwitch, Button, IconButton, List, Typography, SidebarHeader) so it
//! matches the rest of the app's styling. The section rhythm and control
//! metrics follow the design sheet's `.cs-side` block (`ChartStudio` in
//! `screens.html`).

use eframe::egui;
use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonType, IconButton, List, ListEvent, ListItem, ListItemPrefix,
    NumberInput, Select, SelectOption, SidebarHeader, Size, ToggleSwitch, Typography,
    TypographyVariant,
};
use thoth_plugin_sdk::theme::{FIELD_HEIGHT, FONT_CONTROL};

use super::{
    Aggregation, ChartOptions, ChartSpec, ChartType, ColumnInfo, ProducerKind, ProducerRef,
    SortMode, series_palette,
};
use crate::app::tab_manager::TabId;
use crate::theme::{GUTTER_GAP, ThemeColors};

/// Horizontal inset, matching `SidebarHeader`/list rows and the sibling sidebar
/// panels (all of which inset by [`GUTTER_GAP`]).
const PAD_X: f32 = GUTTER_GAP;
/// Trailing inset under the last section — design `.cs-side{padding:0 0 12px}`.
const PAD_BOTTOM: f32 = 12.0;
/// Space above each group label — design `.cs-sec{padding-top:12px}`.
const SECTION_GAP: f32 = 12.0;
/// Gap under a group label — design `.glabel{margin:0 0 7px}`.
const LABEL_GAP: f32 = 7.0;
/// Gap between the field caption and its control — design `.fl{margin-bottom:4px}`.
const CAPTION_GAP: f32 = 4.0;
/// Gap between form rows inside a section — design `.frow{margin-bottom:9px}`.
const ROW_GAP: f32 = 9.0;
/// Gap between the parts of a Y-series row — design `.yrow{gap:7px}`.
const SERIES_GAP: f32 = 7.0;
/// Gap between successive Y-series rows — design `.yrow{margin-bottom:6px}`.
const SERIES_ROW_GAP: f32 = 6.0;
/// Chart-type grid gutter — design `.cs-types{gap:6px}`.
const TYPE_GAP: f32 = 6.0;
/// Glyph inside a chart-type tile — design `.ct{font-size:20px}`.
const TYPE_GLYPH: f32 = 20.0;
/// Upper bound on a chart-type tile, so a wide sidebar doesn't grow huge squares
/// (the tiles are `aspect-ratio:1`, sized by the 4-column grid).
const TYPE_CELL_MAX: f32 = 84.0;
/// Series colour chip — design `.yrow .sw{width:13px;height:13px}`.
const SWATCH: f32 = 13.0;
/// …and its corner radius — design `.sw{border-radius:3px}`. Below the control
/// rung of the radius ladder: this is a 13px decoration, not a control.
const SWATCH_RADIUS: f32 = 3.0;
/// Vertical padding on an options row — design `.togrow{padding:5px 0}`.
const TOGGLE_PAD_Y: f32 = 5.0;
/// Toggle track width, reserved so the switch sits hard right — design
/// `.switch{width:32px}` / `.togrow{justify-content:space-between}`.
const TOGGLE_TRACK_W: f32 = 32.0;
/// Full-width primary action — design `.btn.wide{height:30px}`.
const WIDE_BUTTON_H: f32 = 30.0;

/// What the config panel is asking the app to do.
pub enum ChartStudioEvent {
    /// The user picked a data source; the app should resolve its columns and
    /// feed them back via [`ChartStudio::set_columns`].
    SelectSource(TabId),
    /// Build (or update) a chart tab from this spec.
    Generate(ChartSpec),
    /// Activate an already-open chart tab.
    FocusChart(TabId),
}

#[derive(Default)]
pub struct ChartStudio {
    /// Eligible producer tabs (injected by the app each frame).
    producers: Vec<ProducerRef>,
    /// The currently selected source tab, if any.
    selected: Option<TabId>,
    /// Column schema of the selected source (injected after a resolve).
    columns: Vec<ColumnInfo>,
    chart_type: ChartType,
    x_col: usize,
    y_cols: Vec<usize>,
    options: ChartOptions,
    aggregation: Aggregation,
    top_n: usize,
    sort: SortMode,
    /// Open chart tabs `(tab id, title)` for the "Open Charts" list.
    open_charts: Vec<(TabId, String)>,
    /// When `Some`, the panel is editing this existing chart tab (Generate
    /// updates it in place and reads "Update Chart").
    editing: Option<TabId>,
}

impl ChartStudio {
    /// Refresh the eligible producer list (called by the app each frame).
    /// Drops the selection if the source tab has gone away.
    pub fn set_producers(&mut self, producers: Vec<ProducerRef>) {
        if let Some(sel) = self.selected
            && !producers.iter().any(|p| p.tab_id == sel)
        {
            self.selected = None;
            self.columns.clear();
        }
        self.producers = producers;
    }

    /// Preselect a data source (e.g. when opened via a plugin's "open in
    /// Charts" action). Also leaves any prior edit mode.
    pub fn select_source(&mut self, tab_id: TabId) {
        self.selected = Some(tab_id);
        self.editing = None;
    }

    /// Feed the resolved column schema for the selected source. Resets the axis
    /// selection to sensible defaults (X = first column, Y = first numeric).
    pub fn set_columns(&mut self, columns: Vec<ColumnInfo>) {
        self.x_col = 0;
        let first_numeric = columns.iter().position(|c| c.numeric).unwrap_or(0);
        self.y_cols = vec![first_numeric];
        self.columns = columns;
    }

    /// Load an existing chart's spec + columns for editing.
    pub fn edit(&mut self, spec: ChartSpec, columns: Vec<ColumnInfo>) {
        self.selected = Some(spec.source_tab);
        self.chart_type = spec.chart_type;
        // Clamp indices to the freshly-resolved schema — the source may have
        // fewer columns now than when the chart was created.
        let max_col = columns.len().saturating_sub(1);
        self.x_col = spec.x_col.min(max_col);
        self.y_cols = if spec.y_cols.is_empty() {
            vec![0]
        } else {
            spec.y_cols.into_iter().map(|c| c.min(max_col)).collect()
        };
        self.options = spec.options;
        self.aggregation = spec.aggregation;
        self.top_n = spec.top_n;
        self.sort = spec.sort;
        self.columns = columns;
        self.editing = spec.edit_target;
    }

    /// Update the "Open Charts" list.
    pub fn set_open_charts(&mut self, open: Vec<(TabId, String)>) {
        self.open_charts = open;
    }

    fn numeric_cols(&self) -> Vec<usize> {
        self.columns
            .iter()
            .enumerate()
            .filter(|(_, c)| c.numeric)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn render(&mut self, ui: &mut egui::Ui) -> Vec<ChartStudioEvent> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let mut events = Vec::new();

        // Flush section header (aligns with other sidebar panels).
        ui.add(SidebarHeader::builder().title("CHART STUDIO").build());

        // Everything else is inset to match the list rows' left padding.
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: PAD_X as i8,
                right: PAD_X as i8,
                // Each section opens with its own `SECTION_GAP`; the panel only
                // owns the trailing inset — design `.cs-side{padding:0 0 12px}`.
                top: 0,
                bottom: PAD_BOTTOM as i8,
            })
            .show(ui, |ui| {
                // Every gap in the panel is explicit, straight off the design's
                // `.cs-sec` / `.frow` / `.yrow` / `.togrow` metrics.
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                // Fit the panel width exactly — no horizontal scrolling.
                let width = ui.available_width();

                self.data_source_section(ui, width, &mut events);
                self.chart_type_section(ui, width);

                if !self.columns.is_empty() {
                    self.axes_section(ui, &colors, width);
                    self.data_section(ui, width);
                    self.options_section(ui);
                }

                ui.add_space(SECTION_GAP);
                self.generate_button(ui, &mut events);

                if !self.open_charts.is_empty() {
                    self.open_charts_section(ui, &mut events);
                }
            });

        events
    }

    /// Section heading — design `.glabel`, with `.cs-sec`'s 12px top padding
    /// above it and its own 7px bottom margin below.
    fn group_label(ui: &mut egui::Ui, text: &str) {
        ui.add_space(SECTION_GAP);
        ui.add(
            Typography::builder()
                .text(text)
                .variant(TypographyVariant::GroupLabel)
                .build(),
        );
        ui.add_space(LABEL_GAP);
    }

    /// Caption above a control — design `.frow .fl{font-size:11px;color:overlay1;
    /// margin-bottom:4px}`, i.e. the 11px muted `Caption` variant.
    fn field_label(ui: &mut egui::Ui, text: &str) {
        ui.add(
            Typography::builder()
                .text(text)
                .variant(TypographyVariant::Caption)
                .build(),
        );
        ui.add_space(CAPTION_GAP);
    }

    fn data_source_section(
        &mut self,
        ui: &mut egui::Ui,
        width: f32,
        events: &mut Vec<ChartStudioEvent>,
    ) {
        Self::group_label(ui, "DATA SOURCE");
        if self.producers.is_empty() {
            ui.add(
                Typography::builder()
                    .text("No open data sources. Open a file or a producer plugin.")
                    .variant(TypographyVariant::BodyMuted)
                    .build(),
            );
            return;
        }
        // Flat option list, files first then plugins, each prefixed with a kind
        // glyph (Select has no option groups).
        let mut options = Vec::new();
        for kind in [ProducerKind::File, ProducerKind::Plugin] {
            let glyph = match kind {
                ProducerKind::File => egui_phosphor::regular::FILE,
                ProducerKind::Plugin => egui_phosphor::regular::PLUG,
            };
            for p in self.producers.iter().filter(|p| p.kind == kind) {
                options.push(
                    SelectOption::builder()
                        .value(p.tab_id.to_string())
                        .label(format!("{glyph}  {}", p.label))
                        .build(),
                );
            }
        }
        let mut select = Select::builder()
            .id("chart_ds")
            .value(self.selected.map(|t| t.to_string()).unwrap_or_default())
            .options(options)
            // Design leads the source trigger with an accent database glyph.
            .icon(egui_phosphor::regular::DATABASE)
            .icon_color("accent")
            .width(width)
            .size(Size::Medium)
            .build();
        if let Some(v) = select.show(ui).inner.selected
            && let Ok(tab) = v.parse::<TabId>()
            && self.selected != Some(tab)
        {
            self.selected = Some(tab);
            events.push(ChartStudioEvent::SelectSource(tab));
        }
    }

    /// The 4-column grid of square type tiles — design
    /// `.cs-types{grid-template-columns:repeat(4,1fr);gap:6px}` with `.ct`
    /// tiles (surface + hairline edge, accent fill + glow when `.on`), which is
    /// exactly what a framed/selected `IconButton` paints.
    fn chart_type_section(&mut self, ui: &mut egui::Ui, width: f32) {
        Self::group_label(ui, "CHART TYPE");
        let cols = 4;
        // Never exceed the row width (avoids horizontal overflow on narrow panels).
        let cell =
            ((width - TYPE_GAP * (cols as f32 - 1.0)) / cols as f32).clamp(1.0, TYPE_CELL_MAX);
        for (row, chunk) in ChartType::ALL.chunks(cols).enumerate() {
            if row > 0 {
                ui.add_space(TYPE_GAP);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = TYPE_GAP;
                for &ct in chunk {
                    let clicked = ui
                        .add(
                            IconButton::builder()
                                .icon(ct.icon())
                                .tooltip(ct.label())
                                .frame(true)
                                .selected(self.chart_type == ct)
                                .size_px(cell)
                                .icon_size(TYPE_GLYPH)
                                .build(),
                        )
                        .clicked();
                    if clicked {
                        self.chart_type = ct;
                    }
                }
            });
        }
    }

    fn axes_section(&mut self, ui: &mut egui::Ui, colors: &ThemeColors, width: f32) {
        Self::group_label(ui, "AXES");
        let names: Vec<String> = self.columns.iter().map(|c| c.name.clone()).collect();
        let col_options: Vec<SelectOption> = names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                SelectOption::builder()
                    .value(i.to_string())
                    .label(n)
                    .build()
            })
            .collect();

        Self::field_label(ui, "X Axis");
        self.x_col = self.x_col.min(names.len().saturating_sub(1));
        let mut x_select = Select::builder()
            .id("chart_x")
            .value(self.x_col.to_string())
            .options(col_options)
            .width(width)
            .size(Size::Medium)
            .build();
        if let Some(v) = x_select.show(ui).inner.selected
            && let Ok(i) = v.parse::<usize>()
        {
            self.x_col = i;
        }

        ui.add_space(ROW_GAP);
        // Heatmap plots every numeric column and ignores the Y selection, so
        // don't offer a (no-op) Y picker for it.
        if self.chart_type == ChartType::Heatmap {
            Self::field_label(ui, "Plots all numeric columns");
            return;
        }
        let y_label = if self.chart_type.single_series() {
            "Value"
        } else {
            "Y Series"
        };
        Self::field_label(ui, y_label);

        let numeric = self.numeric_cols();
        let numeric_options: Vec<SelectOption> = numeric
            .iter()
            .map(|&ni| {
                SelectOption::builder()
                    .value(ni.to_string())
                    .label(&names[ni])
                    .build()
            })
            .collect();
        let palette = series_palette(colors);
        if self.chart_type.single_series() {
            self.y_cols.truncate(1);
            if self.y_cols.is_empty() {
                self.y_cols.push(*numeric.first().unwrap_or(&0));
            }
        }

        let multi = !self.chart_type.single_series() && self.y_cols.len() > 1;
        // Leave room for the colour chip (and the remove button when multi) —
        // design `.yrow{gap:7px}` with `.sw` 13px and a 26px `.ib`.
        let remove_w = Size::Medium.metrics().1 + SERIES_GAP;
        let combo_w = width - SWATCH - SERIES_GAP - if multi { remove_w } else { 0.0 };
        let mut remove: Option<usize> = None;
        for i in 0..self.y_cols.len() {
            if i > 0 {
                ui.add_space(SERIES_ROW_GAP);
            }
            ui.horizontal(|ui| {
                Self::series_swatch(ui, palette[i % palette.len()]);
                ui.add_space(SERIES_GAP);
                let mut y_select = Select::builder()
                    .id(format!("chart_y_{i}"))
                    .value(self.y_cols[i].to_string())
                    .options(numeric_options.clone())
                    .width(combo_w)
                    .size(Size::Medium)
                    .build();
                if let Some(v) = y_select.show(ui).inner.selected
                    && let Ok(c) = v.parse::<usize>()
                {
                    self.y_cols[i] = c;
                }
                if multi {
                    ui.add_space(SERIES_GAP);
                    if ui
                        .add(
                            IconButton::builder()
                                .icon(egui_phosphor::regular::X)
                                .tooltip("Remove series")
                                // Design's 26px `.ib`, matching the width
                                // `remove_w` reserves for it above.
                                .size(Size::Medium)
                                .build(),
                        )
                        .clicked()
                    {
                        remove = Some(i);
                    }
                }
            });
        }
        if let Some(i) = remove {
            self.y_cols.remove(i);
        }

        let can_add = !self.chart_type.single_series()
            && self.y_cols.len() < palette.len()
            && !numeric.is_empty();
        if can_add {
            ui.add_space(SERIES_ROW_GAP);
            // Design `<button class="btn text sm">` — a small ghost text button
            // in the muted role, not an accent one.
            let clicked = ui
                .add(
                    Button::builder()
                        .label("Add series")
                        .icon(egui_phosphor::regular::PLUS)
                        .button_type(ButtonType::Text)
                        .button_size(Size::Small)
                        .build(),
                )
                .clicked();
            if clicked {
                let next = numeric
                    .iter()
                    .find(|c| !self.y_cols.contains(c))
                    .copied()
                    .unwrap_or(numeric[0]);
                self.y_cols.push(next);
            }
        }
    }

    /// The series colour chip beside a Y picker — design
    /// `.yrow .sw{width:13px;height:13px;border-radius:3px}`. No SDK component
    /// paints a bare colour chip, so it is a direct fill in the series colour.
    /// Claims the select's full height so the chip centres on it
    /// (design `.yrow{align-items:center}`).
    fn series_swatch(ui: &mut egui::Ui, color: egui::Color32) {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(SWATCH, FIELD_HEIGHT), egui::Sense::hover());
        let chip = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(SWATCH));
        ui.painter().rect_filled(chip, SWATCH_RADIUS, color);
    }

    fn data_section(&mut self, ui: &mut egui::Ui, width: f32) {
        Self::group_label(ui, "TRANSFORM");

        Self::field_label(ui, "Aggregate");
        let agg_opts: Vec<SelectOption> = Aggregation::ALL
            .iter()
            .enumerate()
            .map(|(i, a)| {
                SelectOption::builder()
                    .value(i.to_string())
                    .label(a.label())
                    .build()
            })
            .collect();
        let cur = Aggregation::ALL
            .iter()
            .position(|a| *a == self.aggregation)
            .unwrap_or(0);
        let mut agg_sel = Select::builder()
            .id("chart_agg")
            .value(cur.to_string())
            .options(agg_opts)
            .width(width)
            .size(Size::Medium)
            .build();
        if let Some(v) = agg_sel.show(ui).inner.selected
            && let Ok(i) = v.parse::<usize>()
            && let Some(a) = Aggregation::ALL.get(i)
        {
            self.aggregation = *a;
        }

        ui.add_space(ROW_GAP);
        Self::field_label(ui, "Sort");
        let sort_opts: Vec<SelectOption> = SortMode::ALL
            .iter()
            .enumerate()
            .map(|(i, s)| {
                SelectOption::builder()
                    .value(i.to_string())
                    .label(s.label())
                    .build()
            })
            .collect();
        let cur = SortMode::ALL
            .iter()
            .position(|s| *s == self.sort)
            .unwrap_or(0);
        let mut sort_sel = Select::builder()
            .id("chart_sort")
            .value(cur.to_string())
            .options(sort_opts)
            .width(width)
            .size(Size::Medium)
            .build();
        if let Some(v) = sort_sel.show(ui).inner.selected
            && let Ok(i) = v.parse::<usize>()
            && let Some(s) = SortMode::ALL.get(i)
        {
            self.sort = *s;
        }

        ui.add_space(ROW_GAP);
        Self::field_label(ui, "Top N (0 = all)");
        let mut top = NumberInput::builder()
            .id("chart_topn")
            .value(self.top_n as f64)
            .min(0.0)
            .max(1000.0)
            .build();
        top.show(ui);
        self.top_n = top.value.max(0.0) as usize;
    }

    /// Option rows — design `.togrow{display:flex;justify-content:space-between;
    /// padding:5px 0;font-size:12.5px;color:var(--text)}`: the label reads on the
    /// left in body colour, the switch sits hard right.
    fn options_section(&mut self, ui: &mut egui::Ui) {
        Self::group_label(ui, "OPTIONS");
        let rows = [
            ("Show legend", self.options.legend),
            ("Show gridlines", self.options.grid),
            ("Stacked", self.options.stacked),
            ("Data labels", self.options.data_labels),
        ];
        let mut toggled = [false; 4];
        for (i, (label, enabled)) in rows.iter().enumerate() {
            ui.add_space(TOGGLE_PAD_Y);
            ui.horizontal(|ui| {
                ui.add(
                    Typography::builder()
                        .text(*label)
                        .variant(TypographyVariant::Body)
                        .size(FONT_CONTROL)
                        .build(),
                );
                // Push the switch to the row's right edge without a centred
                // right-to-left layout (which would claim the full pane height).
                let gap = (ui.available_width() - TOGGLE_TRACK_W).max(0.0);
                ui.add_space(gap);
                if ui
                    .add(
                        ToggleSwitch::builder()
                            .id(format!("chart_opt_{i}"))
                            .enabled(*enabled)
                            .build(),
                    )
                    .clicked()
                {
                    toggled[i] = true;
                }
            });
            ui.add_space(TOGGLE_PAD_Y);
        }
        if toggled[0] {
            self.options.legend = !self.options.legend;
        }
        if toggled[1] {
            self.options.grid = !self.options.grid;
        }
        if toggled[2] {
            self.options.stacked = !self.options.stacked;
        }
        if toggled[3] {
            self.options.data_labels = !self.options.data_labels;
        }
    }

    /// The panel's primary action — design `<button class="btn secondary wide">`:
    /// a full-width 30px neutral *surface* button (`ButtonColor::Default` is the
    /// design's `.btn.secondary`), not a filled accent one.
    fn generate_button(&mut self, ui: &mut egui::Ui, events: &mut Vec<ChartStudioEvent>) {
        let ready = self.selected.is_some() && !self.columns.is_empty() && !self.y_cols.is_empty();
        let editing = self.editing.is_some();
        let (label, icon) = if editing {
            ("Update Chart", egui_phosphor::regular::CHECK)
        } else {
            ("Generate Chart", egui_phosphor::regular::CHART_LINE)
        };
        let clicked = ui
            .add_enabled(
                ready,
                Button::builder()
                    .id("chart_generate")
                    .label(label)
                    .icon(icon)
                    .button_type(ButtonType::Elevated)
                    .color(ButtonColor::Default)
                    .full_width(true)
                    .height(WIDE_BUTTON_H)
                    .build(),
            )
            .clicked();
        if clicked
            && let (Some(tab), Some(src)) = (
                self.selected,
                self.selected
                    .and_then(|id| self.producers.iter().find(|p| p.tab_id == id)),
            )
        {
            events.push(ChartStudioEvent::Generate(ChartSpec {
                source_tab: tab,
                source_label: src.label.clone(),
                chart_type: self.chart_type,
                x_col: self.x_col,
                y_cols: self.y_cols.clone(),
                options: self.options,
                aggregation: self.aggregation,
                top_n: self.top_n,
                sort: self.sort,
                edit_target: self.editing,
            }));
            self.editing = None;
        }
        if editing {
            ui.add_space(SERIES_ROW_GAP);
            // Design `.btn.text.sm` — a quiet escape hatch under the CTA.
            let cancelled = ui
                .add(
                    Button::builder()
                        .label("Cancel edit")
                        .button_type(ButtonType::Text)
                        .button_size(Size::Small)
                        .build(),
                )
                .clicked();
            if cancelled {
                self.editing = None;
            }
        }
    }

    fn open_charts_section(&mut self, ui: &mut egui::Ui, events: &mut Vec<ChartStudioEvent>) {
        Self::group_label(ui, "OPEN CHARTS");
        let items: Vec<ListItem> = self
            .open_charts
            .iter()
            .map(|(_, title)| {
                ListItem::builder()
                    .title(title.clone())
                    .prefix(ListItemPrefix::Icon {
                        glyph: egui_phosphor::regular::CHART_LINE.to_string(),
                        color: None,
                    })
                    .build()
            })
            .collect();
        if let Some(ListEvent::ItemClicked(i)) = List::builder().items(items).build().show(ui)
            && let Some((id, _)) = self.open_charts.get(i)
        {
            events.push(ChartStudioEvent::FocusChart(*id));
        }
    }
}
