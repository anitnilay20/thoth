use serde_json::Value;
use thoth_plugin_sdk::{
    components::{
        Colored, Column, Progress, Row, Separator, Size, Spinner, Split, TableView, Tabs,
        Typography, TypographyVariant,
    },
    render_node::RenderNode,
};

use crate::{state::State, ui::widgets::muted, ICON_TABLE, ICON_TREE_STRUCTURE};

pub fn results_view(state: &State) -> RenderNode {
    RenderNode::Tabs(
        Tabs::builder()
            .id("query-output")
            .headers(vec!["Results".into(), "Explain".into()])
            .icons(vec![
                ICON_TABLE.to_string(),
                ICON_TREE_STRUCTURE.to_string(),
            ])
            .size(Size::Small)
            .children(vec![results(state), result_explain(state)])
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
        Some(Ok(result)) => results_table(result),
        Some(Err(msg)) => RenderNode::Row(
            Row::builder()
                .padding(12.0)
                .children(vec![RenderNode::Colored(
                    Colored::builder()
                        .color("#f38ba8")
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
/// statement with no result set — its command tag.
fn results_table(result: &Value) -> RenderNode {
    let columns = result.get("columns").and_then(|c| c.as_array());
    let rows = result.get("rows").and_then(|r| r.as_array());
    let tag = result.get("tag").and_then(|t| t.as_str());

    match (columns, rows) {
        (Some(cols), Some(rows)) if !cols.is_empty() => {
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
            let table_rows: Vec<Vec<RenderNode>> = rows
                .iter()
                .map(|row| {
                    row.as_array()
                        .map(|cs| cs.iter().map(RenderNode::json_cell).collect())
                        .unwrap_or_default()
                })
                .collect();

            let footer = format!(
                "{} row{}{}",
                rows.len(),
                if rows.len() == 1 { "" } else { "s" },
                tag.map(|t| format!("  ·  {t}")).unwrap_or_default()
            );
            RenderNode::Column(
                Column::builder()
                    .gap(4.0)
                    .children(vec![
                        RenderNode::Table(
                            TableView::builder()
                                .headers(headers)
                                .rows(table_rows)
                                .build(),
                        ),
                        RenderNode::Row(
                            Row::builder()
                                .padding(6.0)
                                .children(vec![muted(&footer)])
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
                        .color("#f38ba8")
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

/// Render a Postgres `EXPLAIN (ANALYZE, FORMAT JSON)` result: a summary stat
/// header over a flat, indented list of plan nodes, each with its estimated
/// rows, cost, a run-time bar, and actual time. Postgres-shaped; other engines
/// return a different structure and fall back to raw JSON.
fn explain_plan(result: &Value) -> RenderNode {
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
                RenderNode::Column(Column::builder().gap(0.0).framed(true).children(body).build()),
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

/// The summary strip: total / planning / execution time and the root node type.
fn stats_header(root: &Value, plan: &Value) -> RenderNode {
    let planning = num(root, "Planning Time");
    let execution = num(root, "Execution Time");
    let plan_type = plan.get("Node Type").and_then(|v| v.as_str()).unwrap_or("?");

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
            .children(vec![muted(&label.to_uppercase()), mono_colored(value, color)])
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
    let nt = node.get("Node Type").and_then(|v| v.as_str()).unwrap_or("?");
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
