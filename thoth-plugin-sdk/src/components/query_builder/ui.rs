//! Rendering for [`QueryBuilder`] — design `.qb`.
//!
//! Four lanes sharing one column of labels: Filter, Group by, Compute, Sort.
//! Each lane is a label, a wrapping row of pills, and one add button pinned
//! right, so the whole query reads top-to-bottom. The foot carries the
//! generated SQL, the row limit and Run. The head stays visible when the lanes
//! are hidden and reads the query back in words, so collapsing the builder
//! never hides what is being asked.

use egui::{Align, Layout, Margin, vec2};

use crate::components::{
    Badge, Button, ButtonColor, ButtonGroupItem, ButtonGroups, ButtonType, CodeEditor,
    CodeEditorOutput, ColumnType, IconButton, Input, NumberInput, Select, SelectOption, Size,
    Typography, TypographyVariant,
};
use crate::theme::{
    FONT_CAPTION, RADIUS_CONTROL, ThemeColors, color_to_hex, edge_stroke, hover_text,
};

use super::{
    Aggregate, AggregateFn, Combine, Filter, Operator, QueryBuilder, QueryBuilderOutput,
    QueryError, QueryField, QuerySpec, Sort,
};

// ── Design metrics ────────────────────────────────────────────────────────────

/// Head strip height — design `.qb-head{height:var(--h-tabstrip)}`.
const HEAD_HEIGHT: f32 = 34.0;
/// Width of a lane's label column — design `.lane{grid-template-columns:74px …}`.
const LANE_LABEL_WIDTH: f32 = 74.0;
/// Gap between a lane's three columns — design `.lane{gap:10px}`.
const LANE_GAP: f32 = 10.0;
/// Lane padding — design `.lane{padding:5px 12px}`.
const LANE_PAD_X: i8 = 12;
const LANE_PAD_Y: i8 = 5;
/// Gap between pills in a lane — design `.lane-items{gap:6px}`.
const ITEM_GAP: f32 = 6.0;
/// Pill height — design `.pill{height:var(--h-btn)}`.
const PILL_HEIGHT: f32 = 26.0;
/// Foot padding — design `.qb-foot{padding:7px 12px}`.
const FOOT_PAD_X: i8 = 12;
const FOOT_PAD_Y: i8 = 7;
/// Gap between the foot's controls — design `.qb-foot{gap:10px}`.
const FOOT_GAP: f32 = 10.0;
/// SQL box inset — design `.sqlbox{margin:0 12px 10px}`.
const SQL_MARGIN: i8 = 12;
const SQL_MARGIN_BOTTOM: i8 = 10;
/// SQL text size — design `.sqlhl{font-size:12.5px}`.
const SQL_FONT: f32 = 12.5;
/// Tallest the SQL preview grows before it scrolls.
const SQL_MAX_ROWS: usize = 12;

/// Chrome a [`Select`] trigger adds around its label: padding, the gap before
/// the caret, and the caret itself (design `.trigger{padding:0 10px;gap:7px}`
/// with a 12px caret). A select given no width fills the space it is handed, so
/// inside a pill each one is sized to what it currently says.
const TRIGGER_CHROME: f32 = 10.0 * 2.0 + 7.0 + 12.0;
/// Narrowest and widest a pill dropdown gets, whatever its label.
const TRIGGER_MIN: f32 = 64.0;
const TRIGGER_MAX: f32 = 184.0;
/// Value field width — design `.pill input.v{width:var(--w,96px)}`.
const VALUE_WIDTH: f32 = 96.0;
/// Each half of a two-value field (`between`), which shares that same span.
const VALUE_HALF_WIDTH: f32 = 68.0;
/// Number of columns past which the field dropdown becomes searchable —
/// scrolling a list beats typing only while the list is short, and a file's
/// table can have hundreds of columns.
const SEARCHABLE_FROM: usize = 12;
/// Ceiling on the row limit. The builder always bounds its own result; this
/// bounds how far that bound can be pushed from the spinner.
const MAX_LIMIT: f64 = 1_000_000.0;

impl QueryBuilder {
    /// Draw the builder, editing [`spec`](QueryBuilder::spec) in place.
    ///
    /// The lanes *are* the query — there is no second copy to fall out of step,
    /// and the SQL box below them renders the same spec rather than being an
    /// editable field of its own.
    pub fn show(&mut self, ui: &mut egui::Ui) -> QueryBuilderOutput {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let lanes_id = ui.make_persistent_id((self.id.as_str(), "qb_lanes"));
        let sql_id = ui.make_persistent_id((self.id.as_str(), "qb_sql"));
        // Closed until asked for. The head still reads the query back in
        // words, so a collapsed builder hides the controls, not the question —
        // and a file opens showing its data rather than four empty lanes.
        let mut lanes_open: bool = ui.ctx().data(|d| d.get_temp(lanes_id).unwrap_or(false));
        // Which tab the lanes area is showing. The two are alternatives — a
        // query is either built or written, and the user works in one of them
        // — so they are tabs rather than a disclosure that stacks both.
        let mut on_sql: bool = ui.ctx().data(|d| d.get_temp(sql_id).unwrap_or(false));
        // The id this builder claims ⌘↵ and ⌘/ under — its own, so two
        // builders on one screen are told apart.
        let keys_id = ui.id().with("query-builder-keys");

        // Consumed before the lanes draw, so a focused value field inside a pill
        // does not swallow ⌘↵ on its way past. Only these two: the design also
        // marks the filter button ⌘F, but a host is likely to have spent that
        // key already, and a component that takes one out from under its host
        // breaks something the user cannot see from here.
        //
        // And only when this builder is the one the user is working in.
        // `draw_data_view` puts a builder in every engine-backed tab, so a
        // split dock draws several on one frame: without this the first one
        // rendered took ⌘↵ whichever pane the user was in, and a `CodeEditor`
        // elsewhere — a plugin's SQL pane — never saw the key at all.
        let owns_keys = crate::theme::owns_navigation_keys(ui.ctx(), keys_id);
        let (toggle_key, run_key) = if owns_keys {
            ui.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::COMMAND, egui::Key::Slash),
                    i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter),
                )
            })
        } else {
            (false, false)
        };
        if toggle_key {
            lanes_open = !lanes_open;
        }

        // A lane action from the host's shortcuts. Taken before the lanes
        // draw, and it opens them: adding a clause the user cannot see would
        // be worse than the shortcut doing nothing.
        let mut changed = false;
        let mut queued_run = false;
        if let Some(action) = self.action.take() {
            match action {
                // Toggling is the one action that must not force the lanes
                // open — that is what it is for.
                super::QueryAction::ToggleLanes => lanes_open = !lanes_open,
                super::QueryAction::Run => queued_run = true,
                add => {
                    lanes_open = true;
                    let landed = match add {
                        super::QueryAction::AddFilter => self.push_filter(),
                        super::QueryAction::AddGroupBy => self.push_group_by(),
                        super::QueryAction::AddAggregate => self.push_aggregate(),
                        _ => self.push_sort(),
                    };
                    changed |= landed;
                    // Focus the new pill's first control, so the shortcut
                    // leaves the user where the typing continues rather than
                    // needing a click to get there.
                    if landed {
                        let last = match add {
                            super::QueryAction::AddFilter => self.spec.filters.len(),
                            super::QueryAction::AddGroupBy => self.spec.group_by.len(),
                            super::QueryAction::AddAggregate => self.spec.aggregates.len(),
                            _ => self.spec.sort.len(),
                        } - 1;
                        self.focus_control = focus_target(&self.id, add, last);
                    }
                }
            }
        }
        let mut run = run_key || queued_run;
        let mut add_filter = false;
        let mut sql_edited = false;

        let drawn = ui
            .vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

                if self.head(ui, &colors, lanes_open, &mut on_sql) {
                    lanes_open = !lanes_open;
                }

                if lanes_open {
                    // Compiled once per frame and shared: the foot names an
                    // incomplete lane as soon as it is incomplete, and the SQL box
                    // shows the statement that same failure is holding back.
                    //
                    // When the user has typed their own SQL it is that, not the
                    // lanes, that will run — so it is that the foot judges and the
                    // pane shows.
                    let compiled = self.sql();
                    let overridden = self.sql_override.is_some();

                    // Shown, so the query the typed SQL grew out of is still
                    // readable, but not editable: two editable copies of one query
                    // means the last one touched wins invisibly.
                    if on_sql {
                        let edited = sql_box(ui, &self.id, &compiled, &mut self.sql_override);
                        if let Some(request) = edited.run {
                            let _ = request;
                            run = true;
                        }
                        if edited.changed {
                            changed = true;
                            sql_edited = true;
                        }
                    } else {
                        // Shown, so the query the typed SQL grew out of is
                        // still readable, but not editable: two editable
                        // copies of one query means the last one touched wins
                        // invisibly.
                        ui.add_enabled_ui(!overridden, |ui| {
                            add_filter |= self.lanes(ui, &colors, &mut changed);
                        });
                    }
                    let (ran, revert) = self.foot(ui, &colors, &compiled, &mut changed, overridden);
                    run |= ran;
                    if revert {
                        self.sql_override = None;
                        changed = true;
                        sql_edited = true;
                        on_sql = false;
                    }
                }
            })
            .response
            .rect;
        // Recorded for the next frame, when this builder has to decide whether
        // ⌘↵ was meant for it. A builder that has never been in the pointer's
        // way or clicked in does not take the key — a shortcut that fires in a
        // pane the user is not looking at is worse than one that does nothing.
        crate::theme::claim_navigation_keys(ui, keys_id, drawn);

        if add_filter {
            changed |= self.push_filter();
        }

        // Spent after one frame. `Select::autofocus` only claims focus once,
        // but leaving the target set would re-arm it every time the lanes are
        // reopened.
        if lanes_open {
            self.focus_control = None;
        }

        ui.ctx().data_mut(|d| {
            d.insert_temp(lanes_id, lanes_open);
            d.insert_temp(sql_id, on_sql);
        });

        QueryBuilderOutput {
            changed,
            run,
            sql_edited,
        }
    }

    /// The strip above the lanes. Returns whether the disclosure was clicked.
    fn head(
        &self,
        ui: &mut egui::Ui,
        colors: &ThemeColors,
        lanes_open: bool,
        on_sql: &mut bool,
    ) -> bool {
        let mut toggled = false;
        egui::Frame::new()
            .inner_margin(Margin::symmetric(8, 0))
            .show(ui, |ui| {
                ui.set_min_height(HEAD_HEIGHT);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = ITEM_GAP;
                    toggled = ui
                        .add(
                            Button::builder()
                                .label("Query")
                                .icon(caret(lanes_open))
                                .button_type(ButtonType::Text)
                                .button_size(Size::Small)
                                .hover_text(if lanes_open {
                                    format!("Hide the query builder ({}/)", modifier())
                                } else {
                                    format!("Show the query builder ({}/)", modifier())
                                })
                                .build(),
                        )
                        .clicked();

                    // Builder | SQL — design `.qb-tabs`. Two ways of saying
                    // the same query, and a user works in one of them, so they
                    // are tabs rather than a disclosure that stacks both. Only
                    // while the lanes are open: with them closed there is no
                    // pane for a tab to choose between.
                    if lanes_open {
                        let picked = ButtonGroups::builder()
                            .id(format!("{}_tabs", self.id))
                            .active(if *on_sql { "sql" } else { "builder" })
                            .items(vec![
                                ButtonGroupItem::builder()
                                    .value("builder")
                                    .label("Builder")
                                    .build(),
                                ButtonGroupItem::builder().value("sql").label("SQL").build(),
                            ])
                            .build()
                            .show(ui)
                            .inner;
                        if let Some(tab) = picked {
                            *on_sql = tab == "sql";
                        }
                    }

                    // Collapsed, this strip is the whole query, so the lanes are
                    // read back as chips. Open, they are already on screen.
                    if !lanes_open {
                        for chip in self.summary_chips() {
                            ui.add(
                                Badge::builder()
                                    .label(chip)
                                    .color(color_to_hex(colors.fg_muted))
                                    .outlined(true)
                                    .size(Size::Small)
                                    .build(),
                            );
                        }
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            Badge::builder()
                                .label(format!("{}/", modifier()))
                                .color(color_to_hex(colors.fg_muted))
                                .outlined(true)
                                .size(Size::Small)
                                .build(),
                        );
                        // The head is on screen even when the lanes are
                        // collapsed, so a failure has to be legible here and
                        // not only in the foot — in the error colour, because
                        // "0 rows" and "no such column" must not read alike.
                        if let Some(status) = &self.status {
                            let response = ui.add(
                                Typography::builder()
                                    .text(status.headline())
                                    .variant(TypographyVariant::Caption)
                                    .maybe_color(status.is_failure().then_some("error"))
                                    .build(),
                            );
                            if let Some(detail) = status.detail() {
                                hover_text(response, detail);
                            }
                        }
                    });
                });
            });
        hairline(ui, colors);
        toggled
    }

    /// The query read back as one chip per clause — design `.qchip`.
    fn summary_chips(&self) -> Vec<String> {
        let spec = &self.spec;
        let mut chips = Vec::new();
        if spec.filters.is_empty() {
            chips.push("every record".to_string());
        } else {
            if spec.combine == Combine::Any && spec.filters.len() > 1 {
                chips.push("any of".to_string());
            }
            chips.extend(spec.filters.iter().map(Filter::phrase));
        }
        if !spec.group_by.is_empty() {
            chips.push(format!("by {}", spec.group_by.join(", ")));
        }
        if !spec.aggregates.is_empty() {
            let names: Vec<String> = spec.aggregates.iter().map(Aggregate::output_name).collect();
            chips.push(format!("compute {}", names.join(", ")));
        }
        chips
    }

    /// The four lanes. Returns whether the filter lane's add button was clicked.
    fn lanes(&mut self, ui: &mut egui::Ui, colors: &ThemeColors, edited: &mut bool) -> bool {
        let add_filter = self.filter_lane(ui, colors, edited);
        hairline(ui, colors);
        self.group_lane(ui, colors, edited);
        hairline(ui, colors);
        self.compute_lane(ui, colors, edited);
        hairline(ui, colors);
        self.sort_lane(ui, colors, edited);
        add_filter
    }

    fn filter_lane(&mut self, ui: &mut egui::Ui, colors: &ThemeColors, edited: &mut bool) -> bool {
        let fields = self.fields.clone();
        let id = self.id.clone();
        let focus = self.focus_control.clone();
        let spec = &mut self.spec;
        let mut remove = None;

        let add = lane(
            ui,
            AddButton {
                lane: "Filter",
                label: "Filter",
                icon: egui_phosphor::regular::FUNNEL,
                tooltip: "Add a filter".to_string(),
                kbd: format!("{}F", modifier()),
                enabled: !fields.is_empty(),
            },
            |ui| {
                // The match-all/any switch only earns its space once there is a
                // second filter to combine. It leads the lane rather than
                // sitting in the 74px label column, which cannot hold it.
                if spec.filters.len() > 1 {
                    let picked = ButtonGroups::builder()
                        .id(format!("{id}_combine"))
                        .active(match spec.combine {
                            Combine::All => "all",
                            Combine::Any => "any",
                        })
                        .items(vec![
                            ButtonGroupItem::builder().value("all").label("All").build(),
                            ButtonGroupItem::builder().value("any").label("Any").build(),
                        ])
                        .build()
                        .show(ui)
                        .inner;
                    if let Some(value) = picked {
                        spec.combine = if value == "any" {
                            Combine::Any
                        } else {
                            Combine::All
                        };
                        *edited = true;
                    }
                }

                if spec.filters.is_empty() {
                    lane_hint(ui, "every record");
                }

                for (index, filter) in spec.filters.iter_mut().enumerate() {
                    pill(ui, colors, |ui| {
                        if let Some(name) = field_select_focused(
                            ui,
                            &format!("{id}_f{index}_field"),
                            &filter.field,
                            &fields,
                            focus.as_deref() == Some(format!("{id}_f{index}_field").as_str()),
                        ) {
                            filter.column = column_type(&fields, &name);
                            filter.field = name;
                            // An operator the new column does not support would
                            // compile to nonsense, so it gives way.
                            if !filter.operator.applies_to(filter.column) {
                                filter.operator = Operator::Equals;
                                filter
                                    .values
                                    .resize(Operator::Equals.arity(), String::new());
                            }
                            *edited = true;
                        }

                        let options: Vec<SelectOption> = Operator::all()
                            .iter()
                            .filter(|op| op.applies_to(filter.column))
                            .map(|op| {
                                SelectOption::builder()
                                    .value(format!("{op:?}"))
                                    .label(op.label())
                                    .build()
                            })
                            .collect();
                        if let Some(value) = pill_select(
                            ui,
                            &format!("{id}_f{index}_op"),
                            &format!("{:?}", filter.operator),
                            filter.operator.label(),
                            options,
                        ) && let Some(op) =
                            Operator::all().iter().find(|op| format!("{op:?}") == value)
                        {
                            filter.operator = *op;
                            filter.values.resize(op.arity(), String::new());
                            *edited = true;
                        }

                        // A change of operator changes how many values it takes,
                        // and the fields follow rather than the other way round.
                        filter.values.resize(filter.operator.arity(), String::new());
                        match filter.operator.arity() {
                            0 => {}
                            1 => {
                                let placeholder = if filter.operator == Operator::AnyOf {
                                    "a, b, c"
                                } else {
                                    "value"
                                };
                                *edited |= value_field(
                                    ui,
                                    &format!("{id}_f{index}_v0"),
                                    &mut filter.values[0],
                                    placeholder,
                                    VALUE_WIDTH,
                                );
                            }
                            _ => {
                                let (lower, upper) = filter.values.split_at_mut(1);
                                *edited |= value_field(
                                    ui,
                                    &format!("{id}_f{index}_v0"),
                                    &mut lower[0],
                                    "from",
                                    VALUE_HALF_WIDTH,
                                );
                                conjunction(ui, "and");
                                *edited |= value_field(
                                    ui,
                                    &format!("{id}_f{index}_v1"),
                                    &mut upper[0],
                                    "to",
                                    VALUE_HALF_WIDTH,
                                );
                            }
                        }

                        if remove_button(ui, "Remove this filter") {
                            remove = Some(index);
                        }
                    });
                }
            },
        );

        if let Some(index) = remove {
            spec.filters.remove(index);
            *edited = true;
        }
        add
    }

    fn group_lane(&mut self, ui: &mut egui::Ui, colors: &ThemeColors, edited: &mut bool) {
        let fields = self.fields.clone();
        let id = self.id.clone();
        let focus = self.focus_control.clone();
        let mut remove = None;

        let add = {
            let spec = &mut self.spec;
            lane(
                ui,
                AddButton {
                    lane: "Group by",
                    label: "Field",
                    icon: egui_phosphor::regular::LIST,
                    tooltip: "Group rows by a field".to_string(),
                    kbd: format!("{}G", modifier()),
                    enabled: !fields.is_empty(),
                },
                |ui| {
                    if spec.group_by.is_empty() {
                        lane_hint(ui, "no grouping");
                    }
                    for (index, field) in spec.group_by.iter_mut().enumerate() {
                        pill(ui, colors, |ui| {
                            if let Some(name) = field_select_focused(
                                ui,
                                &format!("{id}_g{index}"),
                                field,
                                &fields,
                                focus.as_deref() == Some(format!("{id}_g{index}").as_str()),
                            ) {
                                *field = name;
                                *edited = true;
                            }
                            if remove_button(ui, "Stop grouping by this field") {
                                remove = Some(index);
                            }
                        });
                    }
                },
            )
        };

        if let Some(index) = remove {
            self.spec.group_by.remove(index);
            *edited = true;
        }
        if add {
            *edited |= self.push_group_by();
        }
    }

    fn compute_lane(&mut self, ui: &mut egui::Ui, colors: &ThemeColors, edited: &mut bool) {
        let fields = self.fields.clone();
        let id = self.id.clone();
        let focus = self.focus_control.clone();
        let mut remove = None;

        let add = {
            let spec = &mut self.spec;
            lane(
                ui,
                AddButton {
                    lane: "Compute",
                    label: "Aggregate",
                    icon: egui_phosphor::regular::CHART_BAR,
                    tooltip: "Add an aggregate".to_string(),
                    kbd: format!("{}{}A", modifier(), shift()),
                    // Counting rows needs no column, so this lane stays usable
                    // even when no field list has arrived yet.
                    enabled: true,
                },
                |ui| {
                    if spec.aggregates.is_empty() {
                        lane_hint(ui, "nothing computed");
                    }
                    for (index, aggregate) in spec.aggregates.iter_mut().enumerate() {
                        pill(ui, colors, |ui| {
                            let options: Vec<SelectOption> = AggregateFn::all()
                                .iter()
                                .map(|f| {
                                    SelectOption::builder()
                                        .value(format!("{f:?}"))
                                        .label(f.label())
                                        .build()
                                })
                                .collect();
                            if let Some(value) = pill_select_focused(
                                ui,
                                &format!("{id}_a{index}_fn"),
                                &format!("{:?}", aggregate.function),
                                aggregate.function.label(),
                                options,
                                focus.as_deref() == Some(format!("{id}_a{index}_fn").as_str()),
                            ) && let Some(function) = AggregateFn::all()
                                .iter()
                                .find(|f| format!("{f:?}") == value)
                            {
                                aggregate.function = *function;
                                if function.takes_field() {
                                    // Carrying an unusable column into `sum`
                                    // would produce an error the user did not
                                    // ask for, so the field is re-chosen.
                                    let usable = fields.iter().any(|f| {
                                        f.name == aggregate.field
                                            && (!function.numeric_only()
                                                || is_numeric(f.column_type))
                                    });
                                    if !usable {
                                        aggregate.field =
                                            default_field(&fields, function.numeric_only());
                                    }
                                }
                                *edited = true;
                            }

                            if aggregate.function.takes_field() {
                                conjunction(ui, "of");
                                // Summing text is not a query worth compiling,
                                // so a numeric-only aggregate is offered only
                                // numeric columns.
                                let offered: Vec<QueryField> = if aggregate.function.numeric_only()
                                {
                                    fields
                                        .iter()
                                        .filter(|f| is_numeric(f.column_type))
                                        .cloned()
                                        .collect()
                                } else {
                                    fields.clone()
                                };
                                if let Some(name) = field_select(
                                    ui,
                                    &format!("{id}_a{index}_field"),
                                    &aggregate.field,
                                    &offered,
                                ) {
                                    aggregate.field = name;
                                    *edited = true;
                                }
                            }

                            if remove_button(ui, "Remove this aggregate") {
                                remove = Some(index);
                            }
                        });
                    }
                },
            )
        };

        if let Some(index) = remove {
            self.spec.aggregates.remove(index);
            *edited = true;
        }
        if add {
            *edited |= self.push_aggregate();
        }
    }

    fn sort_lane(&mut self, ui: &mut egui::Ui, colors: &ThemeColors, edited: &mut bool) {
        let fields = self.sort_keys();
        let id = self.id.clone();
        let focus = self.focus_control.clone();
        let mut remove = None;

        let add = {
            let spec = &mut self.spec;
            lane(
                ui,
                AddButton {
                    lane: "Sort",
                    label: "Sort",
                    icon: egui_phosphor::regular::ARROWS_DOWN_UP,
                    tooltip: "Add a sort key".to_string(),
                    kbd: format!("{}{}S", modifier(), shift()),
                    enabled: !fields.is_empty(),
                },
                |ui| {
                    if spec.sort.is_empty() {
                        lane_hint(ui, "file order");
                    }
                    for (index, sort) in spec.sort.iter_mut().enumerate() {
                        pill(ui, colors, |ui| {
                            if let Some(name) = field_select_focused(
                                ui,
                                &format!("{id}_s{index}"),
                                &sort.field,
                                &fields,
                                focus.as_deref() == Some(format!("{id}_s{index}").as_str()),
                            ) {
                                sort.field = name;
                                *edited = true;
                            }
                            let (glyph, tip) = if sort.descending {
                                (egui_phosphor::regular::ARROW_DOWN, "Largest first")
                            } else {
                                (egui_phosphor::regular::ARROW_UP, "Smallest first")
                            };
                            if ui
                                .add(
                                    IconButton::builder()
                                        .id(format!("{id}_s{index}_dir"))
                                        .icon(glyph)
                                        .frame(false)
                                        .size(Size::Small)
                                        .tooltip(tip)
                                        .build(),
                                )
                                .clicked()
                            {
                                sort.descending = !sort.descending;
                                *edited = true;
                            }
                            if remove_button(ui, "Remove this sort key") {
                                remove = Some(index);
                            }
                        });
                    }
                },
            )
        };

        if let Some(index) = remove {
            self.spec.sort.remove(index);
            *edited = true;
        }
        if add {
            *edited |= self.push_sort();
        }
    }

    /// The columns the sort lane may be pointed at.
    ///
    /// A grouped query does not return the file's columns — it returns the
    /// group keys and the aggregates, under the names `compile` gives them.
    /// Offering the raw fields there produced `ORDER BY "amount"` beside
    /// `GROUP BY "level"`, which DuckDB rejects at run time with nothing said
    /// beforehand, and left "count per service, largest first" — the main
    /// question anyone groups to ask — impossible to build.
    fn sort_keys(&self) -> Vec<QueryField> {
        if self.spec.group_by.is_empty() && self.spec.aggregates.is_empty() {
            return self.fields.clone();
        }
        let typed = |name: &str| {
            self.fields
                .iter()
                .find(|f| f.name == name)
                .map(|f| f.column_type)
                .unwrap_or_default()
        };
        self.spec
            .group_by
            .iter()
            .map(|name| {
                QueryField::builder()
                    .name(name.clone())
                    .column_type(typed(name))
                    .build()
            })
            .chain(self.spec.aggregates.iter().map(|aggregate| {
                // A count is a number whatever it counted; the others carry
                // the type of the column they were computed over.
                let column_type = match aggregate.function {
                    AggregateFn::Count | AggregateFn::DistinctCount => ColumnType::Integer,
                    _ => typed(&aggregate.field),
                };
                QueryField::builder()
                    .name(aggregate.output_name())
                    .column_type(column_type)
                    .build()
            }))
            .collect()
    }

    /// The footer: SQL disclosure, status, limit, Reset and Run. Returns
    /// whether Run was clicked.
    fn foot(
        &mut self,
        ui: &mut egui::Ui,
        colors: &ThemeColors,
        compiled: &Result<String, QueryError>,
        edited: &mut bool,
        overridden: bool,
    ) -> (bool, bool) {
        hairline(ui, colors);
        let mut run = false;
        let mut revert = false;
        egui::Frame::new()
            .inner_margin(Margin::symmetric(FOOT_PAD_X, FOOT_PAD_Y))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = FOOT_GAP;
                    // Copying the statement is useful from either tab — it is
                    // what you paste into a client — so it lives in the foot
                    // rather than inside the SQL pane.
                    if let Ok(sql) = compiled {
                        ui.add(
                            Button::builder()
                                .label("Copy SQL")
                                .icon(egui_phosphor::regular::COPY)
                                .button_type(ButtonType::Text)
                                .button_size(Size::Small)
                                .copy(sql.clone())
                                .hover_text("Copy the statement this query runs")
                                .build(),
                        );
                    }

                    // Only while the typed SQL is driving. Says so plainly and
                    // offers the way back, because the lanes above are
                    // otherwise greyed with no explanation of by what.
                    if overridden {
                        if ui
                            .add(
                                Button::builder()
                                    .label("Use the lanes")
                                    .icon(egui_phosphor::regular::ARROW_U_UP_LEFT)
                                    .button_type(ButtonType::Text)
                                    .button_size(Size::Small)
                                    .hover_text(
                                        "Go back to the lanes, discarding the SQL you typed",
                                    )
                                    .build(),
                            )
                            .clicked()
                        {
                            revert = true;
                        }
                        ui.add(
                            Typography::builder()
                                .text("editing SQL — the lanes are paused")
                                .variant(TypographyVariant::Caption)
                                .build(),
                        );
                    }

                    // Right-to-left so Run sits on the edge; added rightmost
                    // first, the strip reads Limit · Reset · Run on screen.
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let runnable = compiled.is_ok();
                        run = ui
                            .add(
                                Button::builder()
                                    .label("Run")
                                    .icon(egui_phosphor::regular::PLAY)
                                    .color(ButtonColor::Primary)
                                    .enabled(runnable)
                                    .hover_text(if runnable {
                                        format!("Run the query ({}{})", modifier(), return_key())
                                    } else {
                                        "Finish the query before running it".to_string()
                                    })
                                    .build(),
                            )
                            .clicked();

                        if ui
                            .add(
                                Button::builder()
                                    .label("Reset")
                                    .button_type(ButtonType::Text)
                                    .enabled(!self.spec.is_empty())
                                    .hover_text("Clear every lane")
                                    .build(),
                            )
                            .clicked()
                        {
                            // The limit is a preference about how much to fetch
                            // rather than part of the question, so it survives.
                            let limit = self.spec.limit;
                            self.spec = QuerySpec {
                                limit,
                                ..QuerySpec::default()
                            };
                            *edited = true;
                        }

                        let mut limit = NumberInput::builder()
                            .id(format!("{}_limit", self.id))
                            .value(self.spec.limit as f64)
                            .min(1.0)
                            .max(MAX_LIMIT)
                            .step(100.0)
                            .build();
                        let response = limit.show(ui);
                        if response.changed() {
                            self.spec.limit = limit.value.max(1.0) as usize;
                            *edited = true;
                        }
                        hover_text(response, "Most rows the query returns");
                        ui.add(
                            Typography::builder()
                                .text("Limit")
                                .variant(TypographyVariant::PanelHeader)
                                .build(),
                        );

                        // Whatever is left of the strip belongs to the status
                        // line, which is why it is added last. It carries the
                        // compile error and nothing else — the head already
                        // reports what the last run returned, and saying it
                        // twice on one screen is noise, not emphasis.
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            // A lane that cannot compile comes first: it is
                            // why nothing ran, and the run before it is stale
                            // news. Otherwise a failed run says why here,
                            // where the strip is wide enough to read it — the
                            // head only has room for the headline.
                            if let Err(error) = compiled {
                                ui.add(
                                    Typography::builder()
                                        .text(error.message.clone())
                                        .variant(TypographyVariant::Caption)
                                        .color("error")
                                        .build(),
                                );
                            } else if let Some(status) =
                                self.status.as_ref().filter(|s| s.is_failure())
                            {
                                let response = ui.add(
                                    Typography::builder()
                                        .text(status.headline())
                                        .variant(TypographyVariant::Caption)
                                        .color("error")
                                        .build(),
                                );
                                if let Some(detail) = status.detail() {
                                    hover_text(response, detail);
                                }
                            }
                        });
                    });
                });
            });
        (run, revert)
    }

    /// Append a filter on the first available field. Returns whether it did —
    /// a relation with no columns has nothing to filter on.
    ///
    /// These four exist so a lane's add button and the host's keyboard
    /// shortcut for that lane are the same action, not two that drift.
    pub fn push_filter(&mut self) -> bool {
        match self.new_filter() {
            Some(filter) => {
                self.spec.filters.push(filter);
                true
            }
            None => false,
        }
    }

    /// Append a group key on a field not already grouped by.
    pub fn push_group_by(&mut self) -> bool {
        match unused_field(&self.fields, &self.spec.group_by) {
            Some(field) => {
                self.spec.group_by.push(field);
                true
            }
            None => false,
        }
    }

    /// Append a `count(*)`, the one aggregate that needs no field chosen.
    pub fn push_aggregate(&mut self) -> bool {
        self.spec.aggregates.push(Aggregate {
            function: AggregateFn::Count,
            field: String::new(),
        });
        true
    }

    /// Append an ascending sort on a key not already sorted by.
    pub fn push_sort(&mut self) -> bool {
        let used: Vec<String> = self.spec.sort.iter().map(|s| s.field.clone()).collect();
        match unused_field(&self.sort_keys(), &used) {
            Some(field) => {
                self.spec.sort.push(Sort {
                    field,
                    descending: false,
                });
                true
            }
            None => false,
        }
    }

    /// A filter on the first available field, or `None` when there are no
    /// fields to filter on.
    fn new_filter(&self) -> Option<Filter> {
        let field = self.fields.first()?;
        Some(Filter {
            field: field.name.clone(),
            operator: Operator::Equals,
            values: vec![String::new()],
            column: field.column_type,
        })
    }
}

/// What a lane's add button says.
struct AddButton {
    /// The lane's own label, in its 74px column.
    lane: &'static str,
    label: &'static str,
    icon: &'static str,
    tooltip: String,
    enabled: bool,
    /// The lane's keyboard shortcut, shown as a pill on the button.
    kbd: String,
}

/// One lane: a label column, a wrapping row of pills, and the add button on the
/// right. Returns whether the add button was clicked.
fn lane(ui: &mut egui::Ui, add: AddButton, items: impl FnOnce(&mut egui::Ui)) -> bool {
    let mut clicked = false;
    egui::Frame::new()
        .inner_margin(Margin::symmetric(LANE_PAD_X, LANE_PAD_Y))
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = LANE_GAP;
                ui.allocate_ui_with_layout(
                    vec2(LANE_LABEL_WIDTH, PILL_HEIGHT),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.add(
                            Typography::builder()
                                .text(add.lane)
                                .variant(TypographyVariant::PanelHeader)
                                .build(),
                        );
                    },
                );

                // The add button is placed first so the pills can take exactly
                // what is left: laid out right-to-left it lands on the edge, and
                // the wrapping row then fills the space before it.
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    clicked = ui
                        .add(
                            Button::builder()
                                .label(add.label)
                                .icon(add.icon)
                                // Design `.addbtn`: a hairline edge on no fill.
                                // An empty lane then reads as an invitation
                                // rather than as a control that failed to load,
                                // which is what a bare text button looked like.
                                .button_type(ButtonType::Outlined)
                                .button_size(Size::Small)
                                .enabled(add.enabled)
                                .hover_text(add.tooltip)
                                .kbd(add.kbd)
                                .build(),
                        )
                        .clicked();

                    let width = ui.available_width().max(TRIGGER_MIN);
                    ui.allocate_ui_with_layout(
                        vec2(width, PILL_HEIGHT),
                        Layout::left_to_right(Align::Center).with_main_wrap(true),
                        |ui| {
                            ui.spacing_mut().item_spacing = vec2(ITEM_GAP, ITEM_GAP);
                            items(ui);
                        },
                    );
                });
            });
        });
    clicked
}

/// One clause of the query — design `.pill`. Its controls sit flush inside it,
/// so a filter reads as a sentence rather than as three separate inputs.
fn pill(ui: &mut egui::Ui, colors: &ThemeColors, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(colors.surface)
        .corner_radius(RADIUS_CONTROL)
        .stroke(edge_stroke(colors))
        .inner_margin(Margin {
            left: 4,
            right: 2,
            top: 0,
            bottom: 0,
        })
        .show(ui, |ui| {
            ui.set_min_height(PILL_HEIGHT);
            ui.horizontal(|ui| {
                // Design `.pill{gap:1px}` — the parts read as one control.
                ui.spacing_mut().item_spacing.x = 1.0;
                contents(ui);
            });
        });
}

/// A field dropdown inside a pill. Returns the newly-picked field name.
fn field_select(
    ui: &mut egui::Ui,
    id: &str,
    current: &str,
    fields: &[QueryField],
) -> Option<String> {
    field_select_focused(ui, id, current, fields, false)
}

/// As [`field_select`], taking focus when `focus` — for a pill a shortcut has
/// just added, where the point is to carry on from the keyboard.
fn field_select_focused(
    ui: &mut egui::Ui,
    id: &str,
    current: &str,
    fields: &[QueryField],
    focus: bool,
) -> Option<String> {
    let options: Vec<SelectOption> = fields
        .iter()
        .map(|f| {
            SelectOption::builder()
                .value(f.name.clone())
                .label(f.name.clone())
                .build()
        })
        .collect();
    let searchable = options.len() > SEARCHABLE_FROM;
    pill_select_inner(ui, id, current, current, options, searchable, focus)
}

/// Widget id of the control a lane action's new pill should focus.
///
/// The ids are built the same way the lanes build theirs, which is the whole
/// contract: get it wrong and the shortcut adds a pill and focuses nothing.
fn focus_target(id: &str, action: super::QueryAction, index: usize) -> Option<String> {
    Some(match action {
        super::QueryAction::AddFilter => format!("{id}_f{index}_field"),
        super::QueryAction::AddGroupBy => format!("{id}_g{index}"),
        super::QueryAction::AddAggregate => format!("{id}_a{index}_fn"),
        super::QueryAction::AddSort => format!("{id}_s{index}"),
        // Neither adds a pill, so neither has anything to focus.
        super::QueryAction::ToggleLanes | super::QueryAction::Run => return None,
    })
}

/// A dropdown inside a pill, sized to its current label.
fn pill_select(
    ui: &mut egui::Ui,
    id: &str,
    value: &str,
    label: &str,
    options: Vec<SelectOption>,
) -> Option<String> {
    pill_select_inner(ui, id, value, label, options, false, false)
}

/// As [`pill_select`], taking focus when `focus`.
fn pill_select_focused(
    ui: &mut egui::Ui,
    id: &str,
    value: &str,
    label: &str,
    options: Vec<SelectOption>,
    focus: bool,
) -> Option<String> {
    pill_select_inner(ui, id, value, label, options, false, focus)
}

fn pill_select_inner(
    ui: &mut egui::Ui,
    id: &str,
    value: &str,
    label: &str,
    options: Vec<SelectOption>,
    searchable: bool,
    focus: bool,
) -> Option<String> {
    let (font_size, _) = Size::Small.field_metrics();
    let width = trigger_width(ui, label, font_size);
    Select::builder()
        .id(id.to_string())
        .value(value.to_string())
        .options(options)
        .size(Size::Small)
        .width(width)
        .searchable(searchable)
        .autofocus(focus)
        .build()
        .show(ui)
        .inner
        .selected
        .filter(|picked| picked != value)
}

/// Width a trigger needs to show `label` without truncating it, within bounds.
fn trigger_width(ui: &egui::Ui, label: &str, font_size: f32) -> f32 {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(font_size),
        egui::Color32::PLACEHOLDER,
    );
    (galley.size().x + TRIGGER_CHROME).clamp(TRIGGER_MIN, TRIGGER_MAX)
}

/// A value field inside a pill. Returns whether the text changed.
fn value_field(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut String,
    placeholder: &str,
    width: f32,
) -> bool {
    let mut input = Input::builder()
        .id(id.to_string())
        .value(value.clone())
        .placeholder(placeholder)
        .mono(true)
        .size(Size::Small)
        .desired_width(width)
        .build();
    let changed = input.show(ui).inner;
    if changed {
        *value = input.value;
    }
    changed
}

/// The quiet word joining two controls in a pill — design `.pill .conj`.
fn conjunction(ui: &mut egui::Ui, word: &str) {
    ui.add(
        Typography::builder()
            .text(word)
            .variant(TypographyVariant::Mono)
            .color("muted")
            .size(FONT_CAPTION)
            .build(),
    );
}

/// The × that removes a clause — design `.pill .rm`.
fn remove_button(ui: &mut egui::Ui, tooltip: &str) -> bool {
    ui.add(
        IconButton::builder()
            .icon(egui_phosphor::regular::X)
            .frame(false)
            .size(Size::Small)
            .tooltip(tooltip)
            .build(),
    )
    .clicked()
}

/// What an empty lane says instead of nothing — design `.lane-hint`. An empty
/// lane still means something ("every record", "file order"), and saying so is
/// what keeps it from reading as a control that failed to load.
fn lane_hint(ui: &mut egui::Ui, text: &str) {
    ui.add(
        Typography::builder()
            .text(text)
            .variant(TypographyVariant::Mono)
            .color("muted")
            .size(FONT_CAPTION + 0.5)
            .build(),
    );
}

/// The generated SQL — design `.sqlbox`. Read-only: it renders the lanes, so
/// making it editable would put two copies of the query on screen.
///
/// Drawn by the SDK's code editor rather than as plain text, so the statement
/// arrives highlighted and in the same face as the SQL a person writes by hand
/// elsewhere in the app.
/// Draw the SQL pane, and take an edit if the user makes one.
///
/// Editable, and typing in it takes the query over: `override_sql` is filled
/// with what was typed and from then on that is what runs. The lanes reach
/// `WHERE`, `GROUP BY` and `ORDER BY`; `HAVING`, a percentile, a window
/// function and a join are only reachable by writing them.
fn sql_box(
    ui: &mut egui::Ui,
    id: &str,
    compiled: &Result<String, QueryError>,
    override_sql: &mut Option<String>,
) -> CodeEditorOutput {
    // Nothing to show when the lanes do not compile — the foot has already said
    // why, and a stale statement beside that message would contradict it.
    let Ok(sql) = compiled else {
        return CodeEditorOutput::default();
    };
    let mut out = CodeEditorOutput::default();
    egui::Frame::new()
        .outer_margin(Margin {
            left: SQL_MARGIN,
            right: SQL_MARGIN,
            top: 0,
            bottom: SQL_MARGIN_BOTTOM,
        })
        .show(ui, |ui| {
            let mut editor = CodeEditor::builder()
                .id(format!("{id}_sql"))
                .value(sql.clone())
                .syntax("sql")
                .font_size(SQL_FONT)
                // As tall as the statement, within reason: a four-line query
                // should not reserve room for a twenty-line one. A typed query
                // gets room to grow into, since it is being written rather
                // than read.
                .rows(
                    sql.lines()
                        .count()
                        .max(if override_sql.is_some() { 4 } else { 1 })
                        .clamp(1, SQL_MAX_ROWS),
                )
                .build();
            out = editor.show(ui);
            if out.changed {
                // The first keystroke is what hands the query over.
                *override_sql = Some(editor.value.clone());
            }
        });
    out
}

/// The rule between two lanes — design `.lane + .lane{box-shadow:inset 0 1px 0}`.
fn hairline(ui: &mut egui::Ui, colors: &ThemeColors) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 1.0), egui::Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.center().y, edge_stroke(colors));
}

/// The disclosure caret for an open or closed section.
fn caret(open: bool) -> &'static str {
    if open {
        egui_phosphor::regular::CARET_DOWN
    } else {
        egui_phosphor::regular::CARET_RIGHT
    }
}

/// The platform's command-key mark, for the shortcut chips.
///
/// Phosphor, not the Unicode `⌘`: whether a plain-text symbol has a glyph
/// depends on the fonts the machine happens to carry, and the one that had no
/// glyph rendered as an empty box in the middle of a tooltip. Phosphor ships
/// with the app, so every mark is there on every machine.
fn modifier() -> &'static str {
    if cfg!(target_os = "macos") {
        egui_phosphor::regular::COMMAND
    } else {
        "Ctrl+"
    }
}

/// The Shift mark, Phosphor's fat up-arrow rather than the Unicode `⇧`.
fn shift() -> &'static str {
    egui_phosphor::regular::ARROW_FAT_UP
}

/// The Return key's mark, for the same reason.
///
/// The bare elbow arrow, not Phosphor's `key-return`: that one draws the whole
/// key cap, and at chip size the box closes up into a smudge.
fn return_key() -> &'static str {
    egui_phosphor::regular::ARROW_ELBOW_DOWN_LEFT
}

/// Whether a column holds numbers, which is what `sum` and `avg` need.
fn is_numeric(column: ColumnType) -> bool {
    matches!(column, ColumnType::Integer | ColumnType::Float)
}

/// The type of the named column, or text when it is not one of `fields`.
fn column_type(fields: &[QueryField], name: &str) -> ColumnType {
    fields
        .iter()
        .find(|f| f.name == name)
        .map(|f| f.column_type)
        .unwrap_or_default()
}

/// The field an aggregate should default to — the first numeric one when it
/// must be numeric, otherwise simply the first.
fn default_field(fields: &[QueryField], numeric_only: bool) -> String {
    fields
        .iter()
        .find(|f| !numeric_only || is_numeric(f.column_type))
        .map(|f| f.name.clone())
        .unwrap_or_default()
}

/// The first field not already named in `used`, so adding a second group key
/// does not repeat the first. Falls back to the first field when every one is
/// already in use.
fn unused_field(fields: &[QueryField], used: &[String]) -> Option<String> {
    fields
        .iter()
        .map(|f| f.name.clone())
        .find(|name| !used.contains(name))
        .or_else(|| fields.first().map(|f| f.name.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lane_shortcut_adds_the_same_clause_its_button_does() {
        // The button and the shortcut call one method each, so the two cannot
        // drift into adding different things.
        let mut builder = QueryBuilder::builder()
            .relation("data")
            .fields(vec![
                QueryField::builder()
                    .name("level")
                    .column_type(ColumnType::Text)
                    .build(),
                QueryField::builder()
                    .name("ms")
                    .column_type(ColumnType::Integer)
                    .build(),
            ])
            .build();

        assert!(builder.push_filter());
        assert_eq!(builder.spec.filters.len(), 1);
        assert_eq!(builder.spec.filters[0].field, "level");

        assert!(builder.push_group_by());
        assert_eq!(builder.spec.group_by, ["level"]);
        // A second key takes the next field that is not already grouped by.
        assert!(builder.push_group_by());
        assert_eq!(builder.spec.group_by, ["level", "ms"]);
        // With every field already grouped it repeats the first rather than
        // doing nothing — see `unused_field`, whose reasoning is that a
        // control which silently declines looks broken. Worth knowing that a
        // held ⌘G therefore repeats a key rather than stopping.
        assert!(builder.push_group_by());
        assert_eq!(builder.spec.group_by, ["level", "ms", "level"]);

        assert!(builder.push_aggregate());
        assert_eq!(builder.spec.aggregates[0].function, AggregateFn::Count);

        assert!(builder.push_sort());
        assert_eq!(builder.spec.sort.len(), 1);
    }

    #[test]
    fn a_relation_with_no_columns_has_nothing_to_add() {
        // Every lane declines rather than pushing an entry naming a column
        // that does not exist. `push_aggregate` is the exception: `count(*)`
        // needs no field.
        let mut builder = QueryBuilder::default();
        assert!(!builder.push_filter());
        assert!(!builder.push_group_by());
        assert!(!builder.push_sort());
        assert!(builder.push_aggregate());
    }

    #[test]
    fn a_queued_action_opens_the_lanes_and_is_consumed() {
        // The builder starts collapsed, so a shortcut has to open it — adding
        // a clause the user cannot see would be worse than doing nothing.
        let mut builder = QueryBuilder::builder()
            .id("qb-action")
            .relation("data")
            .fields(vec![
                QueryField::builder()
                    .name("level")
                    .column_type(ColumnType::Text)
                    .build(),
            ])
            .build();
        builder.action = Some(crate::components::QueryAction::AddFilter);

        let out = with_ui(|ui| builder.show(ui));

        assert!(out.changed, "the filter landed");
        assert_eq!(builder.spec.filters.len(), 1);
        assert!(builder.action.is_none(), "the action was consumed");

        // A second frame with nothing queued must not add another.
        let out = with_ui(|ui| builder.show(ui));
        assert!(!out.changed);
        assert_eq!(builder.spec.filters.len(), 1);
    }

    #[test]
    fn a_shortcut_leaves_focus_on_the_new_pill() {
        // The point of ⌘F is to carry on from the keyboard; landing a filter
        // and then needing a click to reach its field defeats it.
        let mut builder = QueryBuilder::builder()
            .id("qb-focus")
            .relation("data")
            .fields(vec![
                QueryField::builder()
                    .name("level")
                    .column_type(ColumnType::Text)
                    .build(),
            ])
            .build();

        // The ids these name have to be the ones the lanes actually build.
        for (action, expected) in [
            (
                crate::components::QueryAction::AddFilter,
                "qb-focus_f0_field",
            ),
            (crate::components::QueryAction::AddGroupBy, "qb-focus_g0"),
            (
                crate::components::QueryAction::AddAggregate,
                "qb-focus_a0_fn",
            ),
            (crate::components::QueryAction::AddSort, "qb-focus_s0"),
        ] {
            assert_eq!(
                focus_target("qb-focus", action, 0).as_deref(),
                Some(expected),
                "{action:?} should focus the control it just created"
            );
        }
        // Neither of these adds a pill.
        assert_eq!(
            focus_target("qb", crate::components::QueryAction::ToggleLanes, 0),
            None
        );
        assert_eq!(
            focus_target("qb", crate::components::QueryAction::Run, 0),
            None
        );

        // And the target is spent, not left armed for every later frame.
        builder.action = Some(crate::components::QueryAction::AddFilter);
        with_ui(|ui| builder.show(ui));
        assert!(builder.focus_control.is_none());
    }

    #[test]
    fn toggling_the_lanes_neither_adds_nor_focuses_anything() {
        let mut builder = QueryBuilder::builder().id("qb-toggle").build();
        builder.action = Some(crate::components::QueryAction::ToggleLanes);
        let out = with_ui(|ui| builder.show(ui));
        assert!(!out.changed);
        assert!(builder.spec.is_empty());
        assert!(builder.focus_control.is_none());
    }

    #[test]
    fn the_sql_tab_shows_the_editor_instead_of_the_lanes() {
        // They are alternatives, not a disclosure that stacks both: on the SQL
        // tab the lanes are not drawn at all, so there is one editable copy of
        // the query on screen.
        let mut builder = QueryBuilder::builder()
            .id("qb-tabs")
            .relation("data")
            .fields(vec![
                QueryField::builder()
                    .name("level")
                    .column_type(ColumnType::Text)
                    .build(),
            ])
            .build();
        builder.spec.filters.push(Filter {
            field: "level".to_string(),
            operator: Operator::Equals,
            values: vec!["error".to_string()],
            column: ColumnType::Text,
        });

        // The builder tab draws the lanes, so the filter's field dropdown is
        // laid out and the pane is at least as tall as a lane.
        let on_builder = with_ui(|ui| {
            builder.show(ui);
            ui.min_rect().height()
        });

        // Switching tab is persisted under the builder's id, so set it the way
        // the head does and redraw.
        with_ui(|ui| {
            let sql_id = ui.make_persistent_id(("qb-tabs", "qb_sql"));
            ui.ctx().data_mut(|d| d.insert_temp(sql_id, true));
            // The lanes have to be open for a tab to mean anything.
            let lanes_id = ui.make_persistent_id(("qb-tabs", "qb_lanes"));
            ui.ctx().data_mut(|d| d.insert_temp(lanes_id, true));
            builder.show(ui)
        });

        // Whichever tab is showing, the query itself is untouched — a tab is a
        // way of looking at it, not an edit.
        assert_eq!(builder.spec.filters.len(), 1);
        assert!(on_builder > 0.0);
    }

    #[test]
    fn a_grouped_query_sorts_by_what_it_returns() {
        // A grouped result has the group keys and the aggregates, not the
        // file's columns — `ORDER BY "amount"` beside `GROUP BY "service"` is
        // rejected by the engine, and "count per service, largest first" was
        // impossible to build because `count` was not offered.
        let mut builder = QueryBuilder::builder()
            .fields(vec![
                QueryField::builder()
                    .name("service")
                    .column_type(ColumnType::Text)
                    .build(),
                QueryField::builder()
                    .name("amount")
                    .column_type(ColumnType::Float)
                    .build(),
            ])
            .build();

        // Ungrouped, the lane offers the file's columns, as before.
        let names = |b: &QueryBuilder| -> Vec<String> {
            b.sort_keys().into_iter().map(|f| f.name).collect()
        };
        assert_eq!(names(&builder), ["service", "amount"]);

        builder.spec.group_by = vec!["service".to_string()];
        builder.spec.aggregates = vec![
            Aggregate {
                function: AggregateFn::Count,
                field: String::new(),
            },
            Aggregate {
                function: AggregateFn::Sum,
                field: "amount".to_string(),
            },
        ];
        assert_eq!(names(&builder), ["service", "count", "sum_amount"]);

        // And each carries a type, so the direction control and any future
        // type-led behaviour read the same as they do elsewhere.
        let keys = builder.sort_keys();
        assert_eq!(keys[0].column_type, ColumnType::Text);
        assert_eq!(keys[1].column_type, ColumnType::Integer);
        assert_eq!(keys[2].column_type, ColumnType::Float);

        // The names it offers are the ones the compiler emits.
        builder.relation = "data".to_string();
        builder.spec.sort = vec![Sort {
            field: "count".to_string(),
            descending: true,
        }];
        let sql = builder.sql().expect("compiles");
        assert!(sql.contains(r#"count(*) AS "count""#), "{sql}");
        assert!(sql.contains(r#"ORDER BY "count" DESC"#), "{sql}");
    }

    /// Run one headless frame and hand the closure a real `Ui`, so the widget
    /// is measured and laid out against live font data like it is in the app.
    fn with_ui<R>(f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        // The host registers the icon font; a bare test context has none, and
        // every button in the builder carries a glyph.
        let mut fonts = egui::FontDefinitions::default();
        crate::theme::register_phosphor(&mut fonts);
        ctx.set_fonts(fonts);
        let mut f = Some(f);
        let mut out = None;
        let _ = ctx.run_ui(Default::default(), |ui| {
            if let Some(f) = f.take() {
                out = Some(f(ui));
            }
        });
        out.expect("the test frame ran")
    }

    fn field(name: &str, column_type: ColumnType) -> QueryField {
        QueryField::builder()
            .name(name)
            .column_type(column_type)
            .build()
    }

    fn builder() -> QueryBuilder {
        QueryBuilder::builder()
            .id("qb")
            .relation("events")
            .fields(vec![
                field("level", ColumnType::Text),
                field("amount", ColumnType::Float),
                field("at", ColumnType::Timestamp),
            ])
            .build()
    }

    #[test]
    fn typed_sql_is_what_runs() {
        // The lanes reach WHERE, GROUP BY and ORDER BY. HAVING, a percentile,
        // a window function and a join are only reachable by writing them —
        // so once the user has, that is the query.
        let mut qb = builder();
        qb.spec.filters.push(Filter {
            field: "level".to_string(),
            operator: Operator::Equals,
            values: vec!["error".to_string()],
            column: ColumnType::Text,
        });
        let from_lanes = qb.sql().unwrap();
        assert!(from_lanes.contains("WHERE"), "{from_lanes}");
        assert!(!qb.is_overridden());

        qb.sql_override = Some(
            "SELECT service, count(*) AS n FROM data GROUP BY service HAVING n > 10".to_string(),
        );
        assert!(qb.is_overridden());
        assert_eq!(qb.sql().unwrap(), qb.sql_override.clone().unwrap());

        // Reverting hands it back, unchanged.
        qb.revert();
        assert!(!qb.is_overridden());
        assert_eq!(qb.sql().unwrap(), from_lanes);
    }

    #[test]
    fn a_lane_edit_cannot_reach_the_query_while_sql_is_typed() {
        // Two editable copies of one query means the last one touched wins
        // invisibly. While SQL is typed the lanes are disabled, so a spec
        // change cannot alter what runs.
        let mut qb = builder();
        qb.sql_override = Some("SELECT 1".to_string());
        qb.spec.filters.push(Filter {
            field: "level".to_string(),
            operator: Operator::Equals,
            values: vec!["error".to_string()],
            column: ColumnType::Text,
        });
        assert_eq!(qb.sql().unwrap(), "SELECT 1", "the lanes must not leak in");
    }

    #[test]
    fn an_unrunnable_lane_still_blocks_run_but_typed_sql_does_not() {
        // An incomplete lane is a compiler error and Run stays disabled. Typed
        // SQL is the user's business — DuckDB is the judge of it, and its
        // error is more useful than anything guessed here.
        let mut qb = builder();
        qb.spec.filters.push(Filter {
            field: "level".to_string(),
            operator: Operator::Equals,
            values: vec![String::new()], // nothing entered
            column: ColumnType::Text,
        });
        assert!(qb.sql().is_err(), "an empty value is an incomplete lane");

        qb.sql_override = Some("SELECT nonsense FROM nowhere".to_string());
        assert!(qb.sql().is_ok(), "typed SQL is handed to the engine as-is");
    }

    #[test]
    fn a_frame_that_nobody_touched_reports_no_edit() {
        // The host persists the spec on `changed`, so a frame that merely drew
        // the lanes must not claim one — otherwise every idle frame writes.
        let mut qb = builder();
        qb.spec.filters.push(Filter {
            field: "level".to_string(),
            operator: Operator::Equals,
            values: vec!["error".to_string()],
            column: ColumnType::Text,
        });
        let before = qb.spec.clone();
        let out = with_ui(|ui| qb.show(ui));
        assert!(!out.changed, "an untouched frame reported an edit");
        assert!(!out.run);
        assert_eq!(qb.spec, before);
    }

    #[test]
    fn every_lane_draws_when_the_query_uses_all_of_them() {
        // Exercises each pill shape in one frame — two-value filters, an
        // aggregate over a column, a group key and a sort key — which is where
        // an id clash or a bad borrow would show up.
        let mut qb = builder();
        qb.spec.filters = vec![
            Filter {
                field: "amount".to_string(),
                operator: Operator::Between,
                values: vec!["1".to_string(), "9".to_string()],
                column: ColumnType::Float,
            },
            Filter {
                field: "level".to_string(),
                operator: Operator::IsNotEmpty,
                values: Vec::new(),
                column: ColumnType::Text,
            },
        ];
        qb.spec.group_by = vec!["level".to_string()];
        qb.spec.aggregates = vec![Aggregate {
            function: AggregateFn::Sum,
            field: "amount".to_string(),
        }];
        qb.spec.sort = vec![Sort {
            field: "amount".to_string(),
            descending: true,
        }];
        let out = with_ui(|ui| qb.show(ui));
        assert!(!out.changed);
    }

    #[test]
    fn an_empty_query_still_says_what_it_asks_for() {
        // Collapsed, the head is the only statement of the query on screen, so
        // "no filters" has to read as something rather than as blank space.
        assert_eq!(builder().summary_chips(), vec!["every record".to_string()]);
    }

    #[test]
    fn any_of_is_said_only_once_there_is_a_choice_to_make() {
        let mut qb = builder();
        qb.spec.combine = Combine::Any;
        qb.spec.filters.push(Filter {
            field: "level".to_string(),
            operator: Operator::Equals,
            values: vec!["error".to_string()],
            column: ColumnType::Text,
        });
        // One filter combines with nothing, so "any of" would be noise.
        assert_eq!(qb.summary_chips(), vec!["level is error".to_string()]);

        qb.spec.filters.push(Filter {
            field: "amount".to_string(),
            operator: Operator::GreaterThan,
            values: vec!["10".to_string()],
            column: ColumnType::Float,
        });
        assert_eq!(
            qb.summary_chips(),
            vec![
                "any of".to_string(),
                "level is error".to_string(),
                "amount more than 10".to_string(),
            ]
        );
    }

    #[test]
    fn the_head_reads_back_grouping_and_computation() {
        let mut qb = builder();
        qb.spec.group_by = vec!["level".to_string()];
        qb.spec.aggregates = vec![Aggregate {
            function: AggregateFn::Sum,
            field: "amount".to_string(),
        }];
        assert_eq!(
            qb.summary_chips(),
            vec![
                "every record".to_string(),
                "by level".to_string(),
                "compute sum_amount".to_string(),
            ]
        );
    }

    #[test]
    fn a_new_filter_carries_its_columns_type() {
        // The type decides whether a value is quoted or written as a number, so
        // a filter that forgets it compiles to the wrong SQL.
        let qb = QueryBuilder::builder()
            .id("qb")
            .fields(vec![field("amount", ColumnType::Float)])
            .build();
        let filter = qb.new_filter().expect("a field to filter on");
        assert_eq!(filter.field, "amount");
        assert_eq!(filter.column, ColumnType::Float);
        assert_eq!(filter.values.len(), Operator::Equals.arity());
    }

    #[test]
    fn a_filter_needs_a_field_to_be_about() {
        assert!(QueryBuilder::default().new_filter().is_none());
    }

    #[test]
    fn a_second_group_key_does_not_repeat_the_first() {
        let fields = vec![
            field("level", ColumnType::Text),
            field("at", ColumnType::Timestamp),
        ];
        let used = vec!["level".to_string()];
        assert_eq!(unused_field(&fields, &used).as_deref(), Some("at"));
        // With everything already grouped, repeating beats adding nothing and
        // leaving the button looking broken.
        let all = vec!["level".to_string(), "at".to_string()];
        assert_eq!(unused_field(&fields, &all).as_deref(), Some("level"));
    }

    #[test]
    fn a_numeric_only_aggregate_defaults_to_a_number() {
        // `sum` over text is an error the user never asked for, so switching to
        // it moves the field rather than keeping an unusable one.
        let fields = vec![
            field("level", ColumnType::Text),
            field("amount", ColumnType::Float),
        ];
        assert_eq!(default_field(&fields, true), "amount");
        assert_eq!(default_field(&fields, false), "level");
        assert_eq!(default_field(&[], true), "");
    }

    #[test]
    fn an_unknown_column_is_treated_as_text() {
        let fields = vec![field("amount", ColumnType::Float)];
        assert_eq!(column_type(&fields, "amount"), ColumnType::Float);
        assert_eq!(column_type(&fields, "missing"), ColumnType::Text);
    }

    #[test]
    fn a_pill_dropdown_is_never_wider_than_the_lane_it_sits_in() {
        // A column name can be arbitrarily long; the trigger has to give way
        // rather than push the rest of the pill off screen.
        let long = "a".repeat(400);
        let width = with_ui(|ui| trigger_width(ui, &long, 11.5));
        assert_eq!(width, TRIGGER_MAX);
        let narrow = with_ui(|ui| trigger_width(ui, "id", 11.5));
        assert_eq!(narrow, TRIGGER_MIN);
    }
}
