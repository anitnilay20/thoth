use serde_json::Value;
use thoth_plugin_sdk::{
    components::{
        Colored, Column, ColumnType, Progress, Row, Separator, Size, Spinner, Split, TableView,
        Tabs, Typography, TypographyVariant,
    },
    render_node::RenderNode,
};

use crate::{
    state::State, ui::widgets::muted, ICON_CHART_BAR, ICON_CHAT_TEXT, ICON_TABLE,
    ICON_TREE_STRUCTURE,
};

pub fn results_view(state: &State) -> RenderNode {
    RenderNode::Tabs(
        Tabs::builder()
            .id("query-output")
            .headers(vec![
                "Results".into(),
                "Messages".into(),
                "Explain".into(),
                "Stats".into(),
            ])
            .icons(vec![
                ICON_TABLE.to_string(),
                ICON_CHAT_TEXT.to_string(),
                ICON_TREE_STRUCTURE.to_string(),
                ICON_CHART_BAR.to_string(),
            ])
            .size(Size::Small)
            .children(vec![
                results(state),
                messages(state),
                result_explain(state),
                stats(state),
            ])
            .build(),
    )
}

fn results(st: &State) -> RenderNode {
    if st.loading {
        return RenderNode::Row(
            Row::builder()
                .padding(16.0)
                .gap(10.0)
                .align(thoth_plugin_sdk::components::Align::Center)
                .children(vec![
                    RenderNode::Spinner(Spinner::builder().build()),
                    muted("Running query…"),
                ])
                .build(),
        );
    }
    match &st.result {
        Some(Ok(result)) => results_table(result, st.has_more),
        Some(Err(msg)) => RenderNode::Row(
            Row::builder()
                .padding(12.0)
                .children(vec![RenderNode::Colored(
                    Colored::builder()
                        .color("error")
                        .child(RenderNode::Text(
                            Typography::builder().text(format!("Error: {msg}")).build(),
                        ))
                        .build(),
                )])
                .build(),
        ),
        None => RenderNode::Row(
            Row::builder()
                .padding(12.0)
                .children(vec![muted("Run a query to see results.")])
                .build(),
        ),
    }
}

/// Render a `QueryResult` ({columns, rows, tag}) as a typed table, or — for a
/// statement with no result set — its command tag. `has_more` shows a "Load
/// more" affordance when the run hit the row cap.
fn results_table(result: &Value, has_more: bool) -> RenderNode {
    let columns = result.get("columns").and_then(|c| c.as_array());
    let rows = result.get("rows").and_then(|r| r.as_array());
    let tag = result.get("tag").and_then(|t| t.as_str());

    match (columns, rows) {
        (Some(cols), Some(rows)) if !cols.is_empty() => {
            let col_types: Vec<ColumnType> = cols
                .iter()
                .map(|c| ColumnType::from_sql(c.get("type").and_then(|t| t.as_str()).unwrap_or("")))
                .collect();
            let headers: Vec<String> = cols
                .iter()
                .map(|c| {
                    let name = c.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let ty = c.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if ty.is_empty() {
                        name.to_string()
                    } else {
                        format!("{name}  ·  {ty}")
                    }
                })
                .collect();
            // Style each cell by its column's SQL type (datetime, numeric, uuid,
            // …); numeric/temporal columns are right-aligned by the table.
            let table_rows: Vec<Vec<RenderNode>> = rows
                .iter()
                .map(|row| {
                    row.as_array()
                        .map(|cs| {
                            cs.iter()
                                .enumerate()
                                .map(|(i, v)| {
                                    RenderNode::typed_cell(
                                        v,
                                        col_types.get(i).copied().unwrap_or_default(),
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect();

            let footer = format!(
                "{} row{}{}{}",
                rows.len(),
                if rows.len() == 1 { "" } else { "s" },
                if has_more { " (capped)" } else { "" },
                tag.map(|t| format!("  ·  {t}")).unwrap_or_default()
            );
            let mut footer_row: Vec<RenderNode> = vec![muted(&footer)];
            if has_more {
                footer_row.push(RenderNode::Spacer(
                    thoth_plugin_sdk::components::Spacer::builder()
                        .size(8.0)
                        .build(),
                ));
                footer_row.push(crate::ui::widgets::button(
                    "load-more",
                    "Load more",
                    "Text",
                    "Default",
                    None,
                    true,
                    false,
                ));
            }
            RenderNode::Column(
                Column::builder()
                    .gap(4.0)
                    .children(vec![
                        RenderNode::Table(
                            TableView::builder()
                                .headers(headers)
                                .rows(table_rows)
                                .column_types(col_types)
                                .build(),
                        ),
                        RenderNode::Row(
                            Row::builder()
                                .padding(6.0)
                                .gap(8.0)
                                .align(thoth_plugin_sdk::components::Align::Center)
                                .children(footer_row)
                                .build(),
                        ),
                    ])
                    .build(),
            )
        }
        _ => RenderNode::Row(
            Row::builder()
                .padding(12.0)
                .children(vec![muted(tag.unwrap_or("Query OK"))])
                .build(),
        ),
    }
}

/// The Messages tab: status lines from the last run (command tag, row count, or
/// the error), styled like a server log.
fn messages(st: &State) -> RenderNode {
    let mut lines: Vec<RenderNode> = Vec::new();
    match &st.result {
        Some(Ok(result)) => {
            if let Some(tag) = result.get("tag").and_then(|t| t.as_str()) {
                lines.push(message_line("OK", "success", tag));
            }
            let n = result
                .get("rows")
                .and_then(|r| r.as_array())
                .map(|r| r.len())
                .unwrap_or(0);
            let more = if st.has_more {
                " (capped — Load more for the rest)"
            } else {
                ""
            };
            lines.push(message_line(
                "OK",
                "success",
                &format!("{n} row{} returned{more}", if n == 1 { "" } else { "s" }),
            ));
        }
        Some(Err(msg)) => lines.push(message_line("ERROR", "error", msg)),
        None => lines.push(muted("Run a query to see server messages.")),
    }
    padded(RenderNode::Column(
        Column::builder().gap(4.0).children(lines).build(),
    ))
}

/// A single `[TAG] message` log line.
fn message_line(tag: &str, color: &str, text: &str) -> RenderNode {
    RenderNode::Row(
        Row::builder()
            .gap(8.0)
            .children(vec![
                RenderNode::Text(
                    Typography::builder()
                        .text(format!("[{tag}]"))
                        .variant(TypographyVariant::Mono)
                        .color(color)
                        .build(),
                ),
                RenderNode::Text(
                    Typography::builder()
                        .text(text)
                        .variant(TypographyVariant::Mono)
                        .build(),
                ),
            ])
            .build(),
    )
}

/// The Stats tab: a summary card (sum · min · max · avg · n) per numeric column.
fn stats(st: &State) -> RenderNode {
    let Some(Ok(result)) = &st.result else {
        return padded(muted("Run a query to see column stats."));
    };
    let (Some(cols), Some(rows)) = (
        result.get("columns").and_then(|c| c.as_array()),
        result.get("rows").and_then(|r| r.as_array()),
    ) else {
        return padded(muted("No results."));
    };

    let mut cards: Vec<RenderNode> = Vec::new();
    for (i, col) in cols.iter().enumerate() {
        let ty = ColumnType::from_sql(col.get("type").and_then(|t| t.as_str()).unwrap_or(""));
        if !matches!(ty, ColumnType::Integer | ColumnType::Float) {
            continue;
        }
        let name = col.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let vals: Vec<f64> = rows
            .iter()
            .filter_map(|r| r.as_array()?.get(i))
            .filter_map(cell_f64)
            .collect();
        if vals.is_empty() {
            continue;
        }
        let n = vals.len();
        let sum: f64 = vals.iter().sum();
        let min = vals.iter().copied().fold(f64::INFINITY, f64::min);
        let max = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        cards.push(stat_card(name, sum, min, max, sum / n as f64, n));
    }
    if cards.is_empty() {
        return padded(muted("No numeric columns to summarise."));
    }
    padded(RenderNode::Column(
        Column::builder().gap(10.0).children(cards).build(),
    ))
}

/// One numeric-column summary card.
fn stat_card(name: &str, sum: f64, min: f64, max: f64, avg: f64, n: usize) -> RenderNode {
    let sub = |label: &str, v: f64| mono_colored(&format!("{label} {}", fmt_num(v)), "muted");
    RenderNode::Column(
        Column::builder()
            .framed(true)
            .gap(4.0)
            .children(vec![
                RenderNode::Text(
                    Typography::builder()
                        .text(name)
                        .variant(TypographyVariant::Mono)
                        .color("muted")
                        .build(),
                ),
                mono_colored(&format!("{}  sum", fmt_num(sum)), "number"),
                RenderNode::Row(
                    Row::builder()
                        .gap(16.0)
                        .children(vec![
                            sub("min", min),
                            sub("max", max),
                            sub("avg", avg),
                            mono_colored(&format!("n {n}"), "muted"),
                        ])
                        .build(),
                ),
            ])
            .build(),
    )
}

/// A cell value coerced to `f64` (a JSON number, or a numeric string like a decimal).
fn cell_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

/// Format an `f64` compactly: whole numbers grouped, else two decimals.
fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        fmt_int(v as i64)
    } else {
        format!("{v:.2}")
    }
}

/// Wrap tab content in a 16px-padded box.
fn padded(content: RenderNode) -> RenderNode {
    RenderNode::Row(
        Row::builder()
            .padding(16.0)
            .max_width(true)
            .children(vec![RenderNode::Column(
                Column::builder().gap(0.0).children(vec![content]).build(),
            )])
            .build(),
    )
}

fn result_explain(st: &State) -> RenderNode {
    if st.explain_loading {
        return RenderNode::Row(
            Row::builder()
                .padding(16.0)
                .gap(10.0)
                .align(thoth_plugin_sdk::components::Align::Center)
                .children(vec![
                    RenderNode::Spinner(Spinner::builder().build()),
                    muted("Analyzing query…"),
                ])
                .build(),
        );
    }
    match &st.explain {
        Some(Ok(result)) => explain_plan(result),
        Some(Err(msg)) => RenderNode::Row(
            Row::builder()
                .padding(12.0)
                .children(vec![RenderNode::Colored(
                    Colored::builder()
                        .color("error")
                        .child(RenderNode::Text(
                            Typography::builder().text(format!("Error: {msg}")).build(),
                        ))
                        .build(),
                )])
                .build(),
        ),
        None => RenderNode::Row(
            Row::builder()
                .padding(12.0)
                .children(vec![muted(
                    "Run a query — its EXPLAIN ANALYZE plan shows here.",
                )])
                .build(),
        ),
    }
}

/// Render an `EXPLAIN` result. Postgres (`EXPLAIN (ANALYZE, FORMAT JSON)`) has
/// actual run-time stats; MySQL (`EXPLAIN FORMAT=JSON`) is estimate-only and
/// carries a different `query_block` shape, so it's rendered separately.
/// Anything unrecognised falls back to raw JSON.
fn explain_plan(result: &Value) -> RenderNode {
    if let Some(root) = mysql_root(result) {
        return mysql_plan(&root);
    }
    let Some(root) = explain_root(result) else {
        return RenderNode::Row(
            Row::builder()
                .padding(12.0)
                .children(vec![RenderNode::Text(
                    Typography::builder().text(result.to_string()).build(),
                )])
                .build(),
        );
    };
    let Some(plan) = root.get("Plan") else {
        return RenderNode::Row(
            Row::builder()
                .padding(12.0)
                .children(vec![RenderNode::Text(
                    Typography::builder().text(result.to_string()).build(),
                )])
                .build(),
        );
    };

    // Slowest node time drives the relative timing-bar widths.
    let mut max_ms = 0.0_f64;
    collect_max_ms(plan, &mut max_ms);
    if max_ms <= 0.0 {
        max_ms = 1.0;
    }

    // One columnar row per node, ruled with separators, inside a framed card.
    let mut rows: Vec<RenderNode> = Vec::new();
    plan_rows(&mut rows, plan, 0, max_ms);
    let mut body: Vec<RenderNode> = Vec::with_capacity(rows.len() * 2);
    for (i, r) in rows.into_iter().enumerate() {
        if i > 0 {
            body.push(RenderNode::Separator(Separator::plain()));
        }
        body.push(r);
    }

    RenderNode::Column(
        Column::builder()
            .gap(8.0)
            .children(vec![
                stats_header(root, plan),
                RenderNode::Column(
                    Column::builder()
                        .gap(0.0)
                        .framed(true)
                        .children(body)
                        .build(),
                ),
            ])
            .build(),
    )
}

/// The top `EXPLAIN` object `{ "Plan": {…}, "Planning Time": …, "Execution Time": … }`,
/// dug out of the query result `{ rows: [[ [ {…} ] ] ], … }`.
fn explain_root(result: &Value) -> Option<&Value> {
    result
        .get("rows")?
        .as_array()?
        .first()?
        .as_array()?
        .first()?
        .as_array()?
        .first()
}

// ── MySQL EXPLAIN FORMAT=JSON ───────────────────────────────────────────────

/// The MySQL `EXPLAIN FORMAT=JSON` document, dug out of the single-cell result
/// (the plan comes back as one JSON string column). `None` when the result
/// isn't a MySQL plan, so [`explain_plan`] falls through to the Postgres path.
fn mysql_root(result: &Value) -> Option<Value> {
    let cell = result
        .get("rows")?
        .as_array()?
        .first()?
        .as_array()?
        .first()?;
    let v = match cell {
        Value::String(s) => serde_json::from_str::<Value>(s).ok()?,
        other => other.clone(),
    };
    v.get("query_block").is_some().then_some(v)
}

/// A flattened plan node for MySQL: an operation label, its estimated row
/// count, and its cumulative cost (all estimates — MySQL's JSON has no timing).
struct MysqlRow {
    op: String,
    rows: Option<i64>,
    cost: Option<f64>,
}

/// Render a MySQL plan: an estimate-only stat header (total query cost) over the
/// same framed, indented rows as Postgres, but with a cost bar instead of a
/// run-time bar (MySQL's `FORMAT=JSON` never carries actual times).
fn mysql_plan(root: &Value) -> RenderNode {
    let qb = root.get("query_block").unwrap_or(root);

    let mut rows: Vec<MysqlRow> = Vec::new();
    mysql_walk_block(qb, 0, &mut rows);

    let max_cost = rows
        .iter()
        .filter_map(|r| r.cost)
        .fold(0.0_f64, f64::max)
        .max(1.0);

    let mut body: Vec<RenderNode> = Vec::with_capacity(rows.len() * 2);
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            body.push(RenderNode::Separator(Separator::plain()));
        }
        let rows_txt = r
            .rows
            .map(|n| format!("{} rows", fmt_int(n)))
            .unwrap_or_default();
        let cost_txt = r.cost.map(|c| format!("cost {c:.2}")).unwrap_or_default();
        body.push(RenderNode::Split(
            Split::builder()
                .gap(12.0)
                .widths(vec![340.0, 90.0, 150.0, 250.0])
                .align(thoth_plugin_sdk::components::Align::Center)
                .children(vec![
                    mono(&r.op),
                    mono_colored(&rows_txt, "number"),
                    mono_colored(&cost_txt, "muted"),
                    RenderNode::Progress(
                        Progress::builder()
                            .value((r.cost.unwrap_or(0.0) / max_cost).clamp(0.0, 1.0))
                            .color("info")
                            .height(8.0)
                            .build(),
                    ),
                ])
                .build(),
        ));
    }

    RenderNode::Column(
        Column::builder()
            .gap(8.0)
            .children(vec![
                mysql_stats_header(qb),
                RenderNode::Column(
                    Column::builder()
                        .gap(0.0)
                        .framed(true)
                        .children(body)
                        .build(),
                ),
            ])
            .build(),
    )
}

/// MySQL summary strip: the optimiser's total query cost + an "estimated" note
/// (there is no run time — `FORMAT=JSON` doesn't execute the query).
fn mysql_stats_header(qb: &Value) -> RenderNode {
    let mut stats: Vec<RenderNode> = Vec::new();
    if let Some(cost) = mysql_cost(qb.get("cost_info")) {
        stats.push(stat("Query cost", &format!("{cost:.2}"), "success"));
    }
    stats.push(stat("Plan", "MySQL · estimated", "info"));
    RenderNode::Row(
        Row::builder()
            .padding(12.0)
            .gap(24.0)
            .children(stats)
            .build(),
    )
}

/// Walk a MySQL plan block, appending a [`MysqlRow`] per table access. Blocks
/// nest through operation wrappers (sort/group/distinct), `nested_loop` joins,
/// and materialised subqueries — each descends one indent level.
fn mysql_walk_block(block: &Value, depth: usize, out: &mut Vec<MysqlRow>) {
    // Operation wrappers (filesort, group, distinct) contain a sub-block.
    for (key, label) in [
        ("ordering_operation", "Sort"),
        ("grouping_operation", "Group"),
        ("duplicates_removal", "Distinct"),
    ] {
        if let Some(inner) = block.get(key) {
            out.push(MysqlRow {
                op: format!("{}{label}", "  ".repeat(depth)),
                rows: None,
                cost: None,
            });
            mysql_walk_block(inner, depth + 1, out);
            return;
        }
    }
    // A join: an ordered list of sub-blocks, each wrapping a table.
    if let Some(arr) = block.get("nested_loop").and_then(|v| v.as_array()) {
        for item in arr {
            mysql_walk_block(item, depth, out);
        }
        return;
    }
    // A single table access (also the shape of each nested_loop item).
    if let Some(t) = block.get("table") {
        mysql_walk_table(t, depth, out);
    }
}

/// Append a table-access row, then descend into any subquery materialised from it.
fn mysql_walk_table(t: &Value, depth: usize, out: &mut Vec<MysqlRow>) {
    let name = t.get("table_name").and_then(|v| v.as_str()).unwrap_or("?");
    let access = t.get("access_type").and_then(|v| v.as_str()).unwrap_or("");
    let key = t.get("key").and_then(|v| v.as_str());
    let mut op = format!("{}{access} {name}", "  ".repeat(depth));
    if let Some(k) = key {
        op.push_str(&format!(" ({k})"));
    }
    let rows = t
        .get("rows_examined_per_scan")
        .or_else(|| t.get("rows_produced_per_join"))
        .and_then(|v| v.as_i64());
    let cost = mysql_cost(t.get("cost_info"));
    out.push(MysqlRow { op, rows, cost });

    // A derived/materialised table nests another query block.
    if let Some(inner) = t
        .get("materialized_from_subquery")
        .and_then(|s| s.get("query_block"))
    {
        mysql_walk_block(inner, depth + 1, out);
    }
}

/// A MySQL `cost_info`'s cumulative `prefix_cost` (or `read_cost`), which the
/// server encodes as a JSON string like `"20.25"`.
fn mysql_cost(cost_info: Option<&Value>) -> Option<f64> {
    let ci = cost_info?;
    let raw = ci
        .get("prefix_cost")
        .or_else(|| ci.get("query_cost"))
        .or_else(|| ci.get("read_cost"))?;
    match raw {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

/// The summary strip: total / planning / execution time and the root node type.
fn stats_header(root: &Value, plan: &Value) -> RenderNode {
    let planning = num(root, "Planning Time");
    let execution = num(root, "Execution Time");
    let plan_type = plan
        .get("Node Type")
        .and_then(|v| v.as_str())
        .unwrap_or("?");

    let mut stats: Vec<RenderNode> = Vec::new();
    if let (Some(p), Some(e)) = (planning, execution) {
        stats.push(stat("Total", &format!("{:.1} ms", p + e), "success"));
    }
    if let Some(p) = planning {
        stats.push(stat("Planning", &format!("{p:.1} ms"), "muted"));
    }
    if let Some(e) = execution {
        stats.push(stat("Execution", &format!("{e:.1} ms"), "muted"));
    }
    stats.push(stat("Plan", plan_type, "info"));

    RenderNode::Row(
        Row::builder()
            .padding(12.0)
            .gap(24.0)
            .children(stats)
            .build(),
    )
}

/// A single labelled metric in the [`stats_header`]: an uppercase muted label
/// over a colour-coded mono value, matching the handoff's `Stat` block.
fn stat(label: &str, value: &str, color: &str) -> RenderNode {
    RenderNode::Column(
        Column::builder()
            .gap(2.0)
            .children(vec![
                muted(&label.to_uppercase()),
                mono_colored(value, color),
            ])
            .build(),
    )
}

/// Emit a plan node (then its children) as one columnar row matching the
/// handoff: indented operation · rows · cost · run-time bar · time — each
/// column colour-coded. Rows count and time are peach (`number`), cost is muted,
/// and the bar is tinted by how long the node took.
fn plan_rows(rows: &mut Vec<RenderNode>, node: &Value, depth: usize, max_ms: f64) {
    let op = format!("{}{}", "  ".repeat(depth), node_descriptor(node));
    let row_count = node
        .get("Actual Rows")
        .or_else(|| node.get("Plan Rows"))
        .and_then(|v| v.as_f64())
        .map(|r| fmt_int(r.round() as i64))
        .unwrap_or_default();
    let cost = match (num(node, "Startup Cost"), num(node, "Total Cost")) {
        (Some(s), Some(t)) => format!("cost {s:.2}..{t:.2}"),
        _ => String::new(),
    };
    let ms = node_total_ms(node);

    rows.push(RenderNode::Split(
        Split::builder()
            .gap(12.0)
            .widths(vec![340.0, 90.0, 150.0, 250.0, 72.0])
            .align(thoth_plugin_sdk::components::Align::Center)
            .children(vec![
                mono(&op),
                mono_colored(&format!("{row_count} rows"), "number"),
                mono_colored(&cost, "muted"),
                RenderNode::Progress(
                    Progress::builder()
                        .value((ms / max_ms).clamp(0.0, 1.0))
                        .color(bar_color(ms))
                        .height(8.0)
                        .build(),
                ),
                mono_colored(&format!("{ms:.1} ms"), "number"),
            ])
            .build(),
    ));

    if let Some(children) = node.get("Plans").and_then(|v| v.as_array()) {
        for child in children {
            plan_rows(rows, child, depth + 1, max_ms);
        }
    }
}

/// `Node Type` plus the index it uses or the relation it touches, when present
/// (e.g. `Index Scan users_org_id_idx`, `Seq Scan organizations`).
fn node_descriptor(node: &Value) -> String {
    let nt = node
        .get("Node Type")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    if let Some(idx) = node.get("Index Name").and_then(|v| v.as_str()) {
        return format!("{nt} {idx}");
    }
    if let Some(rel) = node.get("Relation Name").and_then(|v| v.as_str()) {
        return format!("{nt} {rel}");
    }
    nt.to_string()
}

/// A node's actual run time in ms: per-loop time × loop count.
fn node_total_ms(node: &Value) -> f64 {
    let t = num(node, "Actual Total Time").unwrap_or(0.0);
    let loops = num(node, "Actual Loops").unwrap_or(1.0);
    t * loops
}

fn collect_max_ms(node: &Value, max: &mut f64) {
    let m = node_total_ms(node);
    if m > *max {
        *max = m;
    }
    if let Some(children) = node.get("Plans").and_then(|v| v.as_array()) {
        for c in children {
            collect_max_ms(c, max);
        }
    }
}

/// Bar colour by run time, matching the design's thresholds.
fn bar_color(ms: f64) -> &'static str {
    if ms > 100.0 {
        "warning"
    } else if ms > 50.0 {
        "info"
    } else {
        "success"
    }
}

fn num(node: &Value, key: &str) -> Option<f64> {
    node.get(key).and_then(|v| v.as_f64())
}

/// Group an integer with thousands separators (e.g. `38291` → `38,291`).
fn fmt_int(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

/// Monospace text node.
fn mono(text: &str) -> RenderNode {
    RenderNode::Text(
        Typography::builder()
            .text(text)
            .variant(TypographyVariant::Mono)
            .build(),
    )
}

/// Monospace text with a semantic colour applied directly to the [`Typography`]
/// (setting the colour on the text node itself, not via a `Colored` wrapper,
/// which the node's own colour would otherwise override).
fn mono_colored(text: &str, color: &str) -> RenderNode {
    RenderNode::Text(
        Typography::builder()
            .text(text)
            .variant(TypographyVariant::Mono)
            .color(color)
            .build(),
    )
}
