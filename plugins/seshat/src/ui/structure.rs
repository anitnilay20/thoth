//! A table's structure tab, matching the design handoff's `TableStructureView`:
//! a header (icon + schema/name + Rows/Columns/Indexes/Size stats), a sub-tab
//! bar, and the Columns / Indexes / Constraints / Foreign Keys / DDL / Triggers
//! views built from the fetched [`TableDetail`](crate::db::TableDetail).

use thoth_plugin_sdk::components::{
    Align, Badge, BgColor, Code, Colored, Column, Icon, Row, Scroll, Separator, Size, Spacer,
    Spinner, TableView, Tabs, Typography, TypographyVariant,
};
use thoth_plugin_sdk::render_node::RenderNode;

use crate::db::{ColumnInfo, Engine, IndexInfo, TableDetail};
use crate::state::{State, View};
use crate::ui::widgets::muted;
use crate::{
    ICON_CHECK_SQUARE, ICON_CIRCLE, ICON_FINGERPRINT, ICON_KEY, ICON_LIGHTNING, ICON_LINK,
    ICON_LIST_NUMBERS, ICON_TABLE,
};

pub(crate) fn structure_view(st: &State) -> RenderNode {
    let (schema, table) = match &st.view {
        View::Structure { schema, table, .. } => (schema.as_str(), table.as_str()),
        View::Editor => ("", ""),
    };

    match &st.structure {
        None => spinner_pane("Loading structure…"),
        Some(Err(e)) => RenderNode::Column(
            Column::builder()
                .gap(0.0)
                .children(vec![
                    header(schema, table, None),
                    RenderNode::Separator(Separator::plain()),
                    RenderNode::Row(
                        Row::builder()
                            .padding(12.0)
                            .children(vec![RenderNode::Colored(
                                Colored::builder()
                                    .color("error")
                                    .child(RenderNode::Text(
                                        Typography::builder().text(format!("Error: {e}")).build(),
                                    ))
                                    .build(),
                            )])
                            .build(),
                    ),
                ])
                .build(),
        ),
        Some(Ok(detail)) => RenderNode::Column(
            Column::builder()
                .gap(0.0)
                .children(vec![
                    header(schema, table, Some(detail)),
                    RenderNode::Separator(Separator::plain()),
                    tabs(schema, table, detail, st.engine()),
                ])
                .build(),
        ),
    }
}

// ── header ──────────────────────────────────────────────────────────────────

/// The header strip: a table-glyph tile, `schema` over `name`, and the four
/// stats (Rows / Columns / Indexes / Size) pushed to the right.
fn header(schema: &str, table: &str, detail: Option<&TableDetail>) -> RenderNode {
    let mut row: Vec<RenderNode> = vec![
        // Icon tile.
        RenderNode::Row(
            Row::builder()
                .bg_color(BgColor::Surface)
                .padding(9.0)
                .children(vec![RenderNode::Icon(
                    Icon::builder()
                        .glyph(ICON_TABLE)
                        .color("string")
                        .size(20.0)
                        .build(),
                )])
                .build(),
        ),
        // schema · name.
        RenderNode::Column(
            Column::builder()
                .gap(2.0)
                .children(vec![
                    RenderNode::Text(
                        Typography::builder()
                            .text(schema)
                            .variant(TypographyVariant::Mono)
                            .color("muted")
                            .build(),
                    ),
                    RenderNode::Text(
                        Typography::builder()
                            .text(table)
                            .variant(TypographyVariant::Heading)
                            .build(),
                    ),
                ])
                .build(),
        ),
        RenderNode::Spacer(Spacer::builder().size(0.0).build()),
    ];

    if let Some(d) = detail {
        row.push(stat("Rows", &fmt_int(d.row_estimate)));
        row.push(stat("Columns", &d.columns.len().to_string()));
        row.push(stat("Indexes", &d.indexes.len().to_string()));
        row.push(stat("Size", if d.size.is_empty() { "—" } else { &d.size }));
    }

    RenderNode::Row(
        Row::builder()
            .padding(14.0)
            .gap(16.0)
            .align(Align::Center)
            .children(row)
            .build(),
    )
}

/// One labelled metric: an uppercase muted caption over a mono value.
fn stat(label: &str, value: &str) -> RenderNode {
    RenderNode::Column(
        Column::builder()
            .gap(2.0)
            .children(vec![
                RenderNode::Text(
                    Typography::builder()
                        .text(label.to_uppercase())
                        .variant(TypographyVariant::Caption)
                        .color("muted")
                        .build(),
                ),
                RenderNode::Text(
                    Typography::builder()
                        .text(value)
                        .variant(TypographyVariant::Mono)
                        .build(),
                ),
            ])
            .build(),
    )
}

// ── sub-tabs ────────────────────────────────────────────────────────────────

fn tabs(schema: &str, table: &str, d: &TableDetail, engine: Engine) -> RenderNode {
    RenderNode::Tabs(
        Tabs::builder()
            .id("structure-tabs")
            .size(Size::Small)
            .headers(vec![
                "Columns".into(),
                "Indexes".into(),
                "Constraints".into(),
                "Foreign Keys".into(),
                "DDL".into(),
                "Triggers".into(),
            ])
            .children(vec![
                table_pane(columns_table(&d.columns)),
                table_pane(indexes_view(&d.indexes)),
                pane("st-con", constraints_view(&d.columns)),
                pane("st-fk", foreign_keys_view(&d.columns)),
                pane("st-ddl", ddl_view(schema, table, d, engine)),
                pane(
                    "st-trg",
                    empty(ICON_LIGHTNING, "No triggers defined on this table."),
                ),
            ])
            .build(),
    )
}

/// Wrap tab content in a padded, vertical scroll region — for content that
/// grows past the pane (constraint cards, DDL). The inner `Column` keeps the
/// content's parent `ui` vertical.
fn pane(id: &str, content: RenderNode) -> RenderNode {
    RenderNode::Scroll(
        Scroll::builder()
            .id(id)
            .both(false)
            .child(padded(content))
            .build(),
    )
}

/// Wrap a [`TableView`] tab in a bare vertical `Column` — no outer scroll and no
/// padding. The table scrolls itself (both axes) and fills the full available
/// height via `egui_extras`; an outer vertical scroll would hand it infinite
/// height (collapsing it to its row count) and padding would shave off ~a row
/// (forcing an unnecessary scrollbar). The `Column` keeps the parent `ui`
/// vertical — the table breaks if its parent is a horizontal `Row`.
fn table_pane(content: RenderNode) -> RenderNode {
    RenderNode::Column(Column::builder().gap(0.0).children(vec![content]).build())
}

/// A 16px-padded, full-width box around `content`, keeping a vertical parent ui.
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

// ── Columns ───────────────────────────────────────────────────────────────

fn columns_table(cols: &[ColumnInfo]) -> RenderNode {
    let headers = vec![
        String::new(),
        "Column".into(),
        "Type".into(),
        "Nullable".into(),
        "Default".into(),
        "Constraints".into(),
    ];
    let rows: Vec<Vec<RenderNode>> = cols
        .iter()
        .map(|c| {
            let (glyph, color, size) = if c.primary_key {
                (ICON_KEY, "warning", 12.0)
            } else if c.foreign_key.is_some() {
                (ICON_LINK, "info", 12.0)
            } else {
                (ICON_CIRCLE, "muted", 6.0)
            };
            vec![
                RenderNode::Icon(Icon::builder().glyph(glyph).color(color).size(size).build()),
                mono(&c.name, if c.primary_key { "warning" } else { "fg" }),
                mono(&c.data_type, "secondary"),
                if c.nullable {
                    mono("nullable", "number")
                } else {
                    mono("NOT NULL", "muted")
                },
                match &c.default {
                    Some(d) if !d.is_empty() => mono(d, "string"),
                    _ => mono("—", "muted"),
                },
                constraint_badges(c),
            ]
        })
        .collect();
    RenderNode::Table(TableView::builder().headers(headers).rows(rows).build())
}

/// The outlined constraint pills for a column (PK / UNIQUE / FK → …).
fn constraint_badges(c: &ColumnInfo) -> RenderNode {
    let mut badges: Vec<RenderNode> = Vec::new();
    if c.primary_key {
        badges.push(badge("PRIMARY KEY", "warning"));
    }
    if c.unique && !c.primary_key {
        badges.push(badge("UNIQUE", "secondary"));
    }
    if let Some(fk) = &c.foreign_key {
        badges.push(badge(&format!("FK → {fk}"), "info"));
    }
    RenderNode::Row(Row::builder().gap(4.0).children(badges).build())
}

// ── Indexes ───────────────────────────────────────────────────────────────

fn indexes_view(indexes: &[IndexInfo]) -> RenderNode {
    if indexes.is_empty() {
        return empty(ICON_LIST_NUMBERS, "No indexes besides the primary key.");
    }
    let headers = vec!["Index".into(), "Columns".into(), "Unique".into()];
    let rows: Vec<Vec<RenderNode>> = indexes
        .iter()
        .map(|i| {
            vec![
                mono(&i.name, "fg"),
                mono(&format!("({})", i.columns.join(", ")), "string"),
                if i.unique {
                    badge("UNIQUE", "secondary")
                } else {
                    muted("—")
                },
            ]
        })
        .collect();
    RenderNode::Table(TableView::builder().headers(headers).rows(rows).build())
}

// ── Constraints ─────────────────────────────────────────────────────────────

fn constraints_view(cols: &[ColumnInfo]) -> RenderNode {
    let pk: Vec<&str> = cols
        .iter()
        .filter(|c| c.primary_key)
        .map(|c| c.name.as_str())
        .collect();
    let mut cards: Vec<RenderNode> = Vec::new();
    if !pk.is_empty() {
        cards.push(constraint_card(
            ICON_KEY,
            "warning",
            "PRIMARY KEY",
            &format!("({})", pk.join(", ")),
        ));
    }
    for c in cols.iter().filter(|c| c.unique && !c.primary_key) {
        cards.push(constraint_card(
            ICON_FINGERPRINT,
            "secondary",
            "UNIQUE",
            &format!("({})", c.name),
        ));
    }
    for c in cols.iter().filter(|c| c.foreign_key.is_some()) {
        let fk = c.foreign_key.as_deref().unwrap_or("");
        cards.push(constraint_card(
            ICON_CHECK_SQUARE,
            "info",
            "FOREIGN KEY",
            &format!("{} → {fk}", c.name),
        ));
    }
    if cards.is_empty() {
        return empty(ICON_CHECK_SQUARE, "No constraints on this table.");
    }
    RenderNode::Column(Column::builder().gap(12.0).children(cards).build())
}

/// A framed constraint row: icon tile + outlined label + mono body.
fn constraint_card(glyph: &str, color: &str, label: &str, body: &str) -> RenderNode {
    RenderNode::Column(
        Column::builder()
            .framed(true)
            .gap(0.0)
            .children(vec![RenderNode::Row(
                Row::builder()
                    .padding(12.0)
                    .gap(12.0)
                    .align(Align::Center)
                    .children(vec![
                        RenderNode::Row(
                            Row::builder()
                                .bg_color(BgColor::Surface)
                                .padding(7.0)
                                .children(vec![RenderNode::Icon(
                                    Icon::builder().glyph(glyph).color(color).size(14.0).build(),
                                )])
                                .build(),
                        ),
                        badge(label, color),
                        mono(body, "fg"),
                    ])
                    .build(),
            )])
            .build(),
    )
}

// ── Foreign Keys ──────────────────────────────────────────────────────────

fn foreign_keys_view(cols: &[ColumnInfo]) -> RenderNode {
    let fks: Vec<&ColumnInfo> = cols.iter().filter(|c| c.foreign_key.is_some()).collect();
    if fks.is_empty() {
        return empty(ICON_LINK, "No foreign keys on this table.");
    }
    let cards: Vec<RenderNode> = fks
        .iter()
        .map(|c| {
            let fk = c.foreign_key.as_deref().unwrap_or("");
            RenderNode::Column(
                Column::builder()
                    .framed(true)
                    .gap(0.0)
                    .children(vec![RenderNode::Row(
                        Row::builder()
                            .padding(14.0)
                            .gap(12.0)
                            .align(Align::Center)
                            .children(vec![
                                RenderNode::Icon(
                                    Icon::builder()
                                        .glyph(ICON_LINK)
                                        .color("info")
                                        .size(18.0)
                                        .build(),
                                ),
                                mono(&c.name, "warning"),
                                mono("→", "muted"),
                                mono(fk, "string"),
                            ])
                            .build(),
                    )])
                    .build(),
            )
        })
        .collect();
    RenderNode::Column(Column::builder().gap(12.0).children(cards).build())
}

// ── DDL ─────────────────────────────────────────────────────────────────────

fn ddl_view(schema: &str, table: &str, d: &TableDetail, engine: Engine) -> RenderNode {
    // Quote every identifier per the engine's convention so reserved words and
    // special characters produce valid SQL. `q` escapes an identifier; `qtable`
    // is the schema-qualified table name.
    let q = |name: &str| quote_ident(name, engine);
    let qtable = format!("{}.{}", q(schema), q(table));

    let mut ddl = format!("-- {schema}.{table}\nCREATE TABLE {qtable} (\n");
    let lines: Vec<String> = d
        .columns
        .iter()
        .map(|c| {
            // Pad the (unquoted) name to align columns, then quote it.
            let mut line = format!("  {:<16} {}", q(&c.name), c.data_type);
            if !c.nullable {
                line.push_str(" NOT NULL");
            }
            if let Some(def) = &c.default {
                if !def.is_empty() {
                    line.push_str(&format!(" DEFAULT {def}"));
                }
            }
            if c.primary_key {
                line.push_str(" PRIMARY KEY");
            } else if c.unique {
                line.push_str(" UNIQUE");
            }
            if let Some(fk) = &c.foreign_key {
                if let Some((t, col)) = fk.split_once('.') {
                    line.push_str(&format!(" REFERENCES {}({})", q(t), q(col)));
                }
            }
            line
        })
        .collect();
    ddl.push_str(&lines.join(",\n"));
    ddl.push_str("\n);");
    for idx in d
        .indexes
        .iter()
        .filter(|i| !i.name.ends_with("_pkey") && i.name != "PRIMARY")
    {
        let cols = idx
            .columns
            .iter()
            .map(|c| q(c))
            .collect::<Vec<_>>()
            .join(", ");
        ddl.push_str(&format!(
            "\n\nCREATE{} INDEX {}\n  ON {qtable} ({cols});",
            if idx.unique { " UNIQUE" } else { "" },
            q(&idx.name),
        ));
    }

    RenderNode::Column(
        Column::builder()
            .framed(true)
            .gap(0.0)
            .children(vec![RenderNode::Row(
                Row::builder()
                    .padding(12.0)
                    .max_width(true)
                    .children(vec![RenderNode::Code(
                        Code::builder().value(ddl).language("sql").build(),
                    )])
                    .build(),
            )])
            .build(),
    )
}

/// Quote a SQL identifier for `engine` (backticks for MySQL, double-quotes
/// otherwise), doubling the quote char to escape it.
fn quote_ident(name: &str, engine: Engine) -> String {
    let q = if engine == Engine::Mysql { '`' } else { '"' };
    format!("{q}{}{q}", name.replace(q, &format!("{q}{q}")))
}

// ── shared bits ───────────────────────────────────────────────────────────

/// A monospace, semantically-coloured text node.
fn mono(text: &str, color: &str) -> RenderNode {
    RenderNode::Text(
        Typography::builder()
            .text(text)
            .variant(TypographyVariant::Mono)
            .color(color)
            .build(),
    )
}

/// An outlined constraint pill.
fn badge(text: &str, color: &str) -> RenderNode {
    RenderNode::Badge(
        Badge::builder()
            .label(text)
            .color(color)
            .outlined(true)
            .build(),
    )
}

/// A centered empty-state with a large muted glyph and a caption.
fn empty(glyph: &str, title: &str) -> RenderNode {
    RenderNode::Row(
        Row::builder()
            .padding(32.0)
            .gap(10.0)
            .align(Align::Center)
            .children(vec![
                RenderNode::Icon(
                    Icon::builder()
                        .glyph(glyph)
                        .color("muted")
                        .size(28.0)
                        .build(),
                ),
                muted(title),
            ])
            .build(),
    )
}

/// A centered spinner pane (initial load).
fn spinner_pane(label: &str) -> RenderNode {
    RenderNode::Row(
        Row::builder()
            .padding(16.0)
            .gap(10.0)
            .align(Align::Center)
            .children(vec![
                RenderNode::Spinner(Spinner::builder().build()),
                muted(label),
            ])
            .build(),
    )
}

/// Group an integer with thousands separators (e.g. `4823017` → `4,823,017`).
fn fmt_int(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}
