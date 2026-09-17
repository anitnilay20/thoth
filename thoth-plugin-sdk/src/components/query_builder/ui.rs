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
    Badge, Button, ButtonColor, ButtonGroupItem, ButtonGroups, ButtonType, Code, ColumnType,
    IconButton, Input, NumberInput, Select, SelectOption, Size, Typography, TypographyVariant,
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
/// SQL box padding — design `.sqlhl{padding:9px 11px}`.
const SQL_PAD_X: i8 = 11;
const SQL_PAD_Y: i8 = 9;

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
        let mut lanes_open: bool = ui.ctx().data(|d| d.get_temp(lanes_id).unwrap_or(true));
        let mut sql_open: bool = ui.ctx().data(|d| d.get_temp(sql_id).unwrap_or(false));

        // Consumed before the lanes draw, so a focused value field inside a pill
        // does not swallow ⌘↵ on its way past. Only these two: the design also
        // marks the filter button ⌘F, but a host is likely to have spent that
        // key already, and a component that takes one out from under its host
        // breaks something the user cannot see from here.
        let (toggle_key, run_key) = ui.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Slash),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter),
            )
        });
        if toggle_key {
            lanes_open = !lanes_open;
        }
        let mut run = run_key;
        let mut add_filter = false;
        let mut changed = false;

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

            if self.head(ui, &colors, lanes_open) {
                lanes_open = !lanes_open;
            }

            if lanes_open {
                // Compiled once per frame and shared: the foot names an
                // incomplete lane as soon as it is incomplete, and the SQL box
                // shows the statement that same failure is holding back.
                let compiled = self.spec.compile(&self.relation);

                add_filter |= self.lanes(ui, &colors, &mut changed);
                run |= self.foot(ui, &colors, &compiled, &mut sql_open, &mut changed);
                if sql_open {
                    sql_box(ui, &colors, &compiled);
                }
            }
        });

        if add_filter && let Some(filter) = self.new_filter() {
            self.spec.filters.push(filter);
            changed = true;
        }

        ui.ctx().data_mut(|d| {
            d.insert_temp(lanes_id, lanes_open);
            d.insert_temp(sql_id, sql_open);
        });

        QueryBuilderOutput { changed, run }
    }

    /// The strip above the lanes. Returns whether the disclosure was clicked.
    fn head(&self, ui: &mut egui::Ui, colors: &ThemeColors, lanes_open: bool) -> bool {
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
                        if let Some(status) = self.status.as_deref() {
                            ui.add(
                                Typography::builder()
                                    .text(status)
                                    .variant(TypographyVariant::Caption)
                                    .build(),
                            );
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
        let spec = &mut self.spec;
        let mut remove = None;

        let add = lane(
            ui,
            AddButton {
                lane: "Filter",
                label: "Filter",
                icon: egui_phosphor::regular::FUNNEL,
                tooltip: "Add a filter".to_string(),
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
                        if let Some(name) = field_select(
                            ui,
                            &format!("{id}_f{index}_field"),
                            &filter.field,
                            &fields,
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
                    enabled: !fields.is_empty(),
                },
                |ui| {
                    if spec.group_by.is_empty() {
                        lane_hint(ui, "no grouping");
                    }
                    for (index, field) in spec.group_by.iter_mut().enumerate() {
                        pill(ui, colors, |ui| {
                            if let Some(name) =
                                field_select(ui, &format!("{id}_g{index}"), field, &fields)
                            {
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
        if add && let Some(field) = unused_field(&self.fields, &self.spec.group_by) {
            self.spec.group_by.push(field);
            *edited = true;
        }
    }

    fn compute_lane(&mut self, ui: &mut egui::Ui, colors: &ThemeColors, edited: &mut bool) {
        let fields = self.fields.clone();
        let id = self.id.clone();
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
                            if let Some(value) = pill_select(
                                ui,
                                &format!("{id}_a{index}_fn"),
                                &format!("{:?}", aggregate.function),
                                aggregate.function.label(),
                                options,
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
            self.spec.aggregates.push(Aggregate {
                function: AggregateFn::Count,
                field: String::new(),
            });
            *edited = true;
        }
    }

    fn sort_lane(&mut self, ui: &mut egui::Ui, colors: &ThemeColors, edited: &mut bool) {
        let fields = self.fields.clone();
        let id = self.id.clone();
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
                    enabled: !fields.is_empty(),
                },
                |ui| {
                    if spec.sort.is_empty() {
                        lane_hint(ui, "file order");
                    }
                    for (index, sort) in spec.sort.iter_mut().enumerate() {
                        pill(ui, colors, |ui| {
                            if let Some(name) =
                                field_select(ui, &format!("{id}_s{index}"), &sort.field, &fields)
                            {
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
            let used: Vec<String> = self.spec.sort.iter().map(|s| s.field.clone()).collect();
            if let Some(field) = unused_field(&self.fields, &used) {
                self.spec.sort.push(Sort {
                    field,
                    descending: false,
                });
                *edited = true;
            }
        }
    }

    /// The footer: SQL disclosure, status, limit, Reset and Run. Returns
    /// whether Run was clicked.
    fn foot(
        &mut self,
        ui: &mut egui::Ui,
        colors: &ThemeColors,
        compiled: &Result<String, QueryError>,
        sql_open: &mut bool,
        edited: &mut bool,
    ) -> bool {
        hairline(ui, colors);
        let mut run = false;
        egui::Frame::new()
            .inner_margin(Margin::symmetric(FOOT_PAD_X, FOOT_PAD_Y))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = FOOT_GAP;
                    if ui
                        .add(
                            Button::builder()
                                .label("SQL")
                                .icon(caret(*sql_open))
                                .button_type(ButtonType::Text)
                                .button_size(Size::Small)
                                .hover_text("Show the statement these lanes compile to")
                                .build(),
                        )
                        .clicked()
                    {
                        *sql_open = !*sql_open;
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
                                        format!("Run the query ({}↵)", modifier())
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
                        // line, which is why it is added last.
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            match compiled {
                                // An unfinished lane is named here, where the
                                // Run button it disables can be seen.
                                Err(error) => {
                                    ui.add(
                                        Typography::builder()
                                            .text(error.message.clone())
                                            .variant(TypographyVariant::Caption)
                                            .color("error")
                                            .build(),
                                    );
                                }
                                Ok(_) => {
                                    if let Some(status) = self.status.as_deref() {
                                        ui.add(
                                            Typography::builder()
                                                .text(status)
                                                .variant(TypographyVariant::Mono)
                                                .color("muted")
                                                .size(FONT_CAPTION)
                                                .build(),
                                        );
                                    }
                                }
                            }
                        });
                    });
                });
            });
        run
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
                                .enabled(add.enabled)
                                .hover_text(add.tooltip)
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
    pill_select_inner(ui, id, current, current, options, searchable)
}

/// A dropdown inside a pill, sized to its current label.
fn pill_select(
    ui: &mut egui::Ui,
    id: &str,
    value: &str,
    label: &str,
    options: Vec<SelectOption>,
) -> Option<String> {
    pill_select_inner(ui, id, value, label, options, false)
}

fn pill_select_inner(
    ui: &mut egui::Ui,
    id: &str,
    value: &str,
    label: &str,
    options: Vec<SelectOption>,
    searchable: bool,
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
fn sql_box(ui: &mut egui::Ui, colors: &ThemeColors, compiled: &Result<String, QueryError>) {
    // Nothing to show when the lanes do not compile — the foot has already said
    // why, and a stale statement beside that message would contradict it.
    let Ok(sql) = compiled else {
        return;
    };
    egui::Frame::new()
        .outer_margin(Margin {
            left: SQL_MARGIN,
            right: SQL_MARGIN,
            top: 0,
            bottom: SQL_MARGIN_BOTTOM,
        })
        .inner_margin(Margin::symmetric(SQL_PAD_X, SQL_PAD_Y))
        .fill(colors.bg)
        .corner_radius(RADIUS_CONTROL)
        .stroke(edge_stroke(colors))
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    ui.add(
                        Button::builder()
                            .label("Copy SQL")
                            .icon(egui_phosphor::regular::COPY)
                            .button_type(ButtonType::Text)
                            .button_size(Size::Small)
                            .copy(sql.clone())
                            .hover_text("Copy the generated SQL")
                            .build(),
                    );
                    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                        ui.add(Code::builder().value(sql.clone()).language("sql").build());
                    });
                });
            });
        });
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

/// The platform's command-key label, for the shortcut chips.
fn modifier() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl+"
    }
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

    /// Run one headless frame and hand the closure a real `Ui`, so the widget
    /// is measured and laid out against live font data like it is in the app.
    fn with_ui<R>(f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        // The host registers the icon font; a bare test context has none, and
        // every button in the builder carries a glyph.
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        fonts.families.insert(
            egui::FontFamily::Name("phosphor".into()),
            vec!["phosphor".into()],
        );
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
