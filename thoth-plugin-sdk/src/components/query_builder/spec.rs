//! The query a builder describes, and the SQL it compiles to.
//!
//! Kept separate from the UI because it is the part that must be *right*: a
//! mis-quoted identifier or an unescaped value is a broken query at best and an
//! injection at worst, and neither is visible by looking at the lanes.
//!
//! The grammar is deliberately small — filter, group, compute, sort, limit —
//! because those four cover what a person asks of a table, and each maps to one
//! SQL clause. Anything beyond them belongs in the SQL editor, not here.

use serde::{Deserialize, Serialize};

use crate::components::ColumnType;

/// How a filter compares.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    /// Exact match.
    #[default]
    Equals,
    /// Anything but.
    NotEquals,
    /// Any of a comma-separated list.
    AnyOf,
    /// Strictly greater.
    GreaterThan,
    /// Greater or equal.
    AtLeast,
    /// Strictly less.
    LessThan,
    /// Less or equal.
    AtMost,
    /// Inclusive, taking two values.
    Between,
    /// Substring match, case-insensitive.
    Contains,
    /// Prefix match, case-insensitive.
    StartsWith,
    /// No value at all.
    IsEmpty,
    /// Some value.
    IsNotEmpty,
}

impl Operator {
    /// How the operator reads in the lane — the design's wording, which is
    /// plain English rather than SQL.
    pub fn label(self) -> &'static str {
        match self {
            Operator::Equals => "is",
            Operator::NotEquals => "is not",
            Operator::AnyOf => "is any of",
            Operator::GreaterThan => "more than",
            Operator::AtLeast => "at least",
            Operator::LessThan => "less than",
            Operator::AtMost => "at most",
            Operator::Between => "between",
            Operator::Contains => "contains",
            Operator::StartsWith => "starts with",
            Operator::IsEmpty => "is empty",
            Operator::IsNotEmpty => "is not empty",
        }
    }

    /// How many values it takes.
    pub fn arity(self) -> usize {
        match self {
            Operator::IsEmpty | Operator::IsNotEmpty => 0,
            Operator::Between => 2,
            _ => 1,
        }
    }

    /// Whether it applies to a column of this type. Ordering comparisons are
    /// meaningless on unordered text, and substring ones on numbers.
    pub fn applies_to(self, column: ColumnType) -> bool {
        let ordered = matches!(
            column,
            ColumnType::Integer
                | ColumnType::Float
                | ColumnType::Date
                | ColumnType::Time
                | ColumnType::Timestamp
        );
        match self {
            Operator::GreaterThan
            | Operator::AtLeast
            | Operator::LessThan
            | Operator::AtMost
            | Operator::Between => ordered,
            Operator::Contains | Operator::StartsWith => !ordered,
            _ => true,
        }
    }

    /// Every operator, in the order the menu offers them.
    pub fn all() -> &'static [Operator] {
        &[
            Operator::Equals,
            Operator::NotEquals,
            Operator::AnyOf,
            Operator::GreaterThan,
            Operator::AtLeast,
            Operator::LessThan,
            Operator::AtMost,
            Operator::Between,
            Operator::Contains,
            Operator::StartsWith,
            Operator::IsEmpty,
            Operator::IsNotEmpty,
        ]
    }
}

/// One condition in the filter lane.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    /// Column being filtered.
    pub field: String,
    /// How it compares.
    pub operator: Operator,
    /// As typed. Interpreted according to `column`, so a numeric column
    /// compares as a number and a text one as text.
    #[serde(default)]
    pub values: Vec<String>,
    /// The column's type, which decides how a value is written into SQL.
    #[serde(default)]
    pub column: ColumnType,
}

impl Filter {
    /// The condition read back as words, e.g. `status is error`.
    ///
    /// Shared by the collapsed head's chips and by [`QuerySpec::summary`], so
    /// the two can never describe the same filter differently.
    pub fn phrase(&self) -> String {
        match self.operator.arity() {
            0 => format!("{} {}", self.field, self.operator.label()),
            _ => format!(
                "{} {} {}",
                self.field,
                self.operator.label(),
                self.values.join(" and ")
            ),
        }
    }
}

/// Whether every filter must match, or any.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Combine {
    /// Every filter must match.
    #[default]
    All,
    /// Any filter may match.
    Any,
}

/// What to compute over a group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateFn {
    /// How many rows.
    #[default]
    Count,
    /// How many different values.
    DistinctCount,
    /// Total.
    Sum,
    /// Mean.
    Average,
    /// Smallest.
    Minimum,
    /// Largest.
    Maximum,
}

impl AggregateFn {
    /// How it reads in the lane.
    pub fn label(self) -> &'static str {
        match self {
            AggregateFn::Count => "Count of rows",
            AggregateFn::DistinctCount => "Distinct values",
            AggregateFn::Sum => "Sum",
            AggregateFn::Average => "Average",
            AggregateFn::Minimum => "Minimum",
            AggregateFn::Maximum => "Maximum",
        }
    }

    /// Whether it needs a column. Counting rows does not.
    pub fn takes_field(self) -> bool {
        !matches!(self, AggregateFn::Count)
    }

    /// Whether it only makes sense on numbers.
    pub fn numeric_only(self) -> bool {
        matches!(self, AggregateFn::Sum | AggregateFn::Average)
    }

    /// Every aggregate, in the order the menu offers them.
    pub fn all() -> &'static [AggregateFn] {
        &[
            AggregateFn::Count,
            AggregateFn::DistinctCount,
            AggregateFn::Sum,
            AggregateFn::Average,
            AggregateFn::Minimum,
            AggregateFn::Maximum,
        ]
    }
}

/// One entry in the compute lane.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aggregate {
    /// What to compute.
    pub function: AggregateFn,
    /// Column to compute over. Ignored for [`AggregateFn::Count`].
    #[serde(default)]
    pub field: String,
}

impl Aggregate {
    /// The column name the result appears under, e.g. `sum_amount`.
    pub fn output_name(&self) -> String {
        if !self.function.takes_field() {
            return "count".to_string();
        }
        let verb = match self.function {
            AggregateFn::DistinctCount => "distinct",
            AggregateFn::Sum => "sum",
            AggregateFn::Average => "avg",
            AggregateFn::Minimum => "min",
            AggregateFn::Maximum => "max",
            AggregateFn::Count => "count",
        };
        format!("{verb}_{}", sanitize(&self.field))
    }
}

/// One entry in the sort lane.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sort {
    /// Column to order by.
    pub field: String,
    /// Largest first.
    #[serde(default)]
    pub descending: bool,
}

/// Everything the lanes describe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuerySpec {
    /// Conditions rows must satisfy.
    #[serde(default)]
    pub filters: Vec<Filter>,
    /// Whether all of them must hold, or any.
    #[serde(default)]
    pub combine: Combine,
    /// Columns to group by.
    #[serde(default)]
    pub group_by: Vec<String>,
    /// What to compute per group.
    #[serde(default)]
    pub aggregates: Vec<Aggregate>,
    /// Ordering keys, applied in turn.
    #[serde(default)]
    pub sort: Vec<Sort>,
    /// Rows returned. A builder always bounds its own result — an unbounded
    /// query against a file-sized table is not something to reach by accident.
    pub limit: usize,
}

impl Default for QuerySpec {
    fn default() -> Self {
        Self {
            filters: Vec::new(),
            combine: Combine::All,
            group_by: Vec::new(),
            aggregates: Vec::new(),
            sort: Vec::new(),
            limit: 1000,
        }
    }
}

/// Why a spec could not be compiled, in the user's terms rather than SQL's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryError {
    /// What went wrong, phrased for the person who built the query.
    pub message: String,
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl QuerySpec {
    /// Whether the spec asks for anything at all.
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
            && self.group_by.is_empty()
            && self.aggregates.is_empty()
            && self.sort.is_empty()
    }

    /// The query read back as a sentence, for the collapsed head.
    ///
    /// The point of the head is that the question stays visible when the lanes
    /// are hidden, so this describes intent rather than syntax.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.filters.is_empty() {
            let joiner = match self.combine {
                Combine::All => " and ",
                Combine::Any => " or ",
            };
            parts.push(
                self.filters
                    .iter()
                    .map(Filter::phrase)
                    .collect::<Vec<_>>()
                    .join(joiner),
            );
        }
        if !self.group_by.is_empty() {
            parts.push(format!("by {}", self.group_by.join(", ")));
        }
        if !self.sort.is_empty() {
            let keys: Vec<String> = self
                .sort
                .iter()
                .map(|s| {
                    format!(
                        "{}{}",
                        s.field,
                        if s.descending { " descending" } else { "" }
                    )
                })
                .collect();
            parts.push(format!("sorted by {}", keys.join(", ")));
        }
        if parts.is_empty() {
            "everything".to_string()
        } else {
            parts.join(" · ")
        }
    }

    /// Compile to SQL against `relation`.
    ///
    /// Identifiers are quoted and literals escaped here, in one place, because
    /// doing it at each call site is how one gets missed.
    pub fn compile(&self, relation: &str) -> Result<String, QueryError> {
        let grouped = !self.group_by.is_empty() || !self.aggregates.is_empty();

        let select = if grouped {
            let mut columns: Vec<String> = self.group_by.iter().map(|f| ident(f)).collect();
            for aggregate in &self.aggregates {
                columns.push(format!(
                    "{} AS {}",
                    aggregate_sql(aggregate)?,
                    ident(&aggregate.output_name())
                ));
            }
            if columns.is_empty() {
                "*".to_string()
            } else {
                columns.join(", ")
            }
        } else {
            "*".to_string()
        };

        let mut sql = format!("SELECT {select}\nFROM {}", ident(relation));

        if !self.filters.is_empty() {
            let glue = match self.combine {
                Combine::All => "\n  AND ",
                Combine::Any => "\n   OR ",
            };
            let conditions: Result<Vec<String>, QueryError> =
                self.filters.iter().map(filter_sql).collect();
            sql.push_str(&format!("\nWHERE {}", conditions?.join(glue)));
        }
        if !self.group_by.is_empty() {
            let keys: Vec<String> = self.group_by.iter().map(|f| ident(f)).collect();
            sql.push_str(&format!("\nGROUP BY {}", keys.join(", ")));
        }
        if !self.sort.is_empty() {
            let keys: Vec<String> = self
                .sort
                .iter()
                .map(|s| {
                    format!(
                        "{} {}",
                        ident(&s.field),
                        if s.descending { "DESC" } else { "ASC" }
                    )
                })
                .collect();
            sql.push_str(&format!("\nORDER BY {}", keys.join(", ")));
        }
        sql.push_str(&format!("\nLIMIT {}", self.limit.max(1)));
        Ok(sql)
    }
}

fn aggregate_sql(aggregate: &Aggregate) -> Result<String, QueryError> {
    if !aggregate.function.takes_field() {
        return Ok("count(*)".to_string());
    }
    if aggregate.field.trim().is_empty() {
        return Err(QueryError {
            message: format!(
                "\"{}\" needs a field — choose one or remove it.",
                aggregate.function.label()
            ),
        });
    }
    let column = ident(&aggregate.field);
    Ok(match aggregate.function {
        AggregateFn::DistinctCount => format!("count(DISTINCT {column})"),
        AggregateFn::Sum => format!("sum({column})"),
        AggregateFn::Average => format!("avg({column})"),
        AggregateFn::Minimum => format!("min({column})"),
        AggregateFn::Maximum => format!("max({column})"),
        AggregateFn::Count => "count(*)".to_string(),
    })
}

fn filter_sql(filter: &Filter) -> Result<String, QueryError> {
    let column = ident(&filter.field);
    let need = |position: usize, which: &str| -> Result<String, QueryError> {
        let raw = filter.values.get(position).map(|v| v.trim()).unwrap_or("");
        if raw.is_empty() {
            return Err(QueryError {
                message: format!(
                    "\"{} {}\" is missing {which} — fill it in or remove the filter.",
                    filter.field,
                    filter.operator.label()
                ),
            });
        }
        literal(raw, filter.column).ok_or_else(|| QueryError {
            message: format!(
                "\"{raw}\" is not a number, and \"{}\" holds numbers.",
                filter.field
            ),
        })
    };

    Ok(match filter.operator {
        Operator::IsEmpty => format!("{column} IS NULL"),
        Operator::IsNotEmpty => format!("{column} IS NOT NULL"),
        Operator::Equals => format!("{column} = {}", need(0, "a value")?),
        Operator::NotEquals => format!("{column} <> {}", need(0, "a value")?),
        Operator::GreaterThan => format!("{column} > {}", need(0, "a value")?),
        Operator::AtLeast => format!("{column} >= {}", need(0, "a value")?),
        Operator::LessThan => format!("{column} < {}", need(0, "a value")?),
        Operator::AtMost => format!("{column} <= {}", need(0, "a value")?),
        Operator::Between => format!(
            "{column} BETWEEN {} AND {}",
            need(0, "a lower bound")?,
            need(1, "an upper bound")?
        ),
        // Substring matching is always textual, whatever the column claims.
        // The ESCAPE clause is required: without it a backslash is just a
        // backslash, and a user's literal `%` would still act as a wildcard.
        Operator::Contains => format!(
            "{column}::VARCHAR ILIKE {} ESCAPE '\\'",
            text_literal(&format!("%{}%", escape_like(&raw(filter, 0, "a value")?)))
        ),
        Operator::StartsWith => format!(
            "{column}::VARCHAR ILIKE {} ESCAPE '\\'",
            text_literal(&format!("{}%", escape_like(&raw(filter, 0, "a value")?)))
        ),
        Operator::AnyOf => {
            let listed = raw(filter, 0, "a value")?;
            let entries: Vec<&str> = listed
                .split(',')
                .map(str::trim)
                .filter(|e| !e.is_empty())
                .collect();
            if entries.is_empty() {
                return Err(QueryError {
                    message: format!(
                        "\"{} is any of\" is missing a value — fill it in or remove the filter.",
                        filter.field
                    ),
                });
            }
            let literals: Result<Vec<String>, QueryError> = entries
                .iter()
                .map(|entry| {
                    literal(entry, filter.column).ok_or_else(|| QueryError {
                        message: format!(
                            "\"{entry}\" is not a number, and \"{}\" holds numbers.",
                            filter.field
                        ),
                    })
                })
                .collect();
            format!("{column} IN ({})", literals?.join(", "))
        }
    })
}

/// A filter's raw nth value, or a message naming what is missing.
fn raw(filter: &Filter, position: usize, which: &str) -> Result<String, QueryError> {
    let value = filter.values.get(position).map(|v| v.trim()).unwrap_or("");
    if value.is_empty() {
        return Err(QueryError {
            message: format!(
                "\"{} {}\" is missing {which} — fill it in or remove the filter.",
                filter.field,
                filter.operator.label()
            ),
        });
    }
    Ok(value.to_string())
}

/// A value as a SQL literal, typed by its column.
///
/// `None` when the column holds numbers and the value is not one — which is a
/// user error worth naming, not a query to send and let fail.
fn literal(value: &str, column: ColumnType) -> Option<String> {
    match column {
        ColumnType::Integer | ColumnType::Float => {
            value.parse::<f64>().ok().map(|_| value.to_string())
        }
        ColumnType::Boolean => match value.to_ascii_lowercase().as_str() {
            "true" | "t" | "1" | "yes" => Some("TRUE".to_string()),
            "false" | "f" | "0" | "no" => Some("FALSE".to_string()),
            _ => Some(text_literal(value)),
        },
        _ => Some(text_literal(value)),
    }
}

/// Quote an identifier, doubling any embedded quote.
fn ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Quote a string literal, doubling any embedded quote.
fn text_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Escape a value used inside a LIKE pattern, so a user's `%` matches a
/// literal `%` rather than acting as a wildcard.
fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// A name reduced to something usable as a column alias.
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(field: &str, operator: Operator, values: &[&str]) -> Filter {
        Filter {
            field: field.to_string(),
            operator,
            values: values.iter().map(|v| v.to_string()).collect(),
            column: ColumnType::Text,
        }
    }

    fn number(field: &str, operator: Operator, values: &[&str]) -> Filter {
        Filter {
            column: ColumnType::Integer,
            ..text(field, operator, values)
        }
    }

    #[test]
    fn an_empty_spec_selects_everything_bounded() {
        let sql = QuerySpec::default().compile("users").unwrap();
        assert_eq!(sql, "SELECT *\nFROM \"users\"\nLIMIT 1000");
        assert!(QuerySpec::default().is_empty());
    }

    #[test]
    fn filters_compile_to_a_where_clause() {
        let spec = QuerySpec {
            filters: vec![
                text("level", Operator::Equals, &["ERROR"]),
                number("attempts", Operator::AtLeast, &["3"]),
            ],
            ..Default::default()
        };
        let sql = spec.compile("logs").unwrap();
        assert!(sql.contains("WHERE \"level\" = 'ERROR'"));
        assert!(sql.contains("AND \"attempts\" >= 3"));
    }

    #[test]
    fn combining_with_any_uses_or() {
        let spec = QuerySpec {
            filters: vec![
                text("level", Operator::Equals, &["ERROR"]),
                text("level", Operator::Equals, &["FATAL"]),
            ],
            combine: Combine::Any,
            ..Default::default()
        };
        let sql = spec.compile("logs").unwrap();
        assert!(sql.contains("OR"), "got: {sql}");
        assert!(!sql.contains("AND"));
    }

    #[test]
    fn a_value_is_typed_by_its_column() {
        // The same text means different SQL depending on the column, which is
        // the whole reason a filter carries its type.
        let numeric = number("id", Operator::Equals, &["42"]).clone();
        let textual = text("code", Operator::Equals, &["42"]).clone();

        assert!(filter_sql(&numeric).unwrap().ends_with("= 42"));
        assert!(filter_sql(&textual).unwrap().ends_with("= '42'"));
    }

    #[test]
    fn a_non_numeric_value_on_a_numeric_column_is_named_not_sent() {
        let spec = QuerySpec {
            filters: vec![number("amount", Operator::GreaterThan, &["lots"])],
            ..Default::default()
        };
        let error = spec.compile("t").unwrap_err();
        assert!(error.message.contains("not a number"), "{}", error.message);
        assert!(error.message.contains("amount"));
    }

    #[test]
    fn a_missing_value_names_what_is_missing() {
        let spec = QuerySpec {
            filters: vec![text("level", Operator::Equals, &[])],
            ..Default::default()
        };
        assert!(spec.compile("t").unwrap_err().message.contains("missing"));

        // `between` names which bound, because "a value" would be ambiguous.
        let spec = QuerySpec {
            filters: vec![number("n", Operator::Between, &["1"])],
            ..Default::default()
        };
        let message = spec.compile("t").unwrap_err().message;
        assert!(message.contains("upper bound"), "{message}");
    }

    #[test]
    fn operators_without_values_need_none() {
        let spec = QuerySpec {
            filters: vec![text("note", Operator::IsEmpty, &[])],
            ..Default::default()
        };
        assert!(spec.compile("t").unwrap().contains("\"note\" IS NULL"));
    }

    #[test]
    fn grouping_and_aggregating_build_the_select_list() {
        let spec = QuerySpec {
            group_by: vec!["level".to_string()],
            aggregates: vec![
                Aggregate {
                    function: AggregateFn::Count,
                    field: String::new(),
                },
                Aggregate {
                    function: AggregateFn::Sum,
                    field: "amount".to_string(),
                },
            ],
            ..Default::default()
        };
        let sql = spec.compile("logs").unwrap();
        assert!(sql.starts_with(
            "SELECT \"level\", count(*) AS \"count\", sum(\"amount\") AS \"sum_amount\""
        ));
        assert!(sql.contains("GROUP BY \"level\""));
    }

    #[test]
    fn an_aggregate_that_needs_a_field_says_so() {
        let spec = QuerySpec {
            aggregates: vec![Aggregate {
                function: AggregateFn::Average,
                field: String::new(),
            }],
            ..Default::default()
        };
        let message = spec.compile("t").unwrap_err().message;
        assert!(message.contains("needs a field"), "{message}");
    }

    #[test]
    fn sorting_applies_keys_in_order() {
        let spec = QuerySpec {
            sort: vec![
                Sort {
                    field: "level".into(),
                    descending: false,
                },
                Sort {
                    field: "ts".into(),
                    descending: true,
                },
            ],
            ..Default::default()
        };
        let sql = spec.compile("logs").unwrap();
        assert!(sql.contains("ORDER BY \"level\" ASC, \"ts\" DESC"));
    }

    // ── Quoting, which is where this would go wrong silently ────────────────

    #[test]
    fn identifiers_with_quotes_cannot_break_out() {
        let spec = QuerySpec {
            filters: vec![text("we\"ird", Operator::Equals, &["x"])],
            ..Default::default()
        };
        let sql = spec.compile("ta\"ble").unwrap();
        assert!(sql.contains("FROM \"ta\"\"ble\""));
        assert!(sql.contains("\"we\"\"ird\" ="));
    }

    #[test]
    fn a_value_cannot_terminate_its_own_literal() {
        // The injection this compiler exists to prevent.
        let spec = QuerySpec {
            filters: vec![text("name", Operator::Equals, &["'; DROP TABLE users; --"])],
            ..Default::default()
        };
        let sql = spec.compile("t").unwrap();
        assert!(sql.contains("'''; DROP TABLE users; --'"), "got: {sql}");
        // One clause, not two statements.
        assert_eq!(sql.matches("SELECT").count(), 1);
    }

    #[test]
    fn like_wildcards_in_a_value_match_literally() {
        // A user typing "50%" means the text, not "anything after 50".
        let spec = QuerySpec {
            filters: vec![text("note", Operator::Contains, &["50%_off"])],
            ..Default::default()
        };
        let sql = spec.compile("t").unwrap();
        assert!(sql.contains("'%50\\%\\_off%'"), "got: {sql}");
        // Escaping only means anything with the clause that enables it.
        assert!(sql.contains("ESCAPE"), "got: {sql}");
    }

    #[test]
    fn any_of_splits_on_commas_and_types_each() {
        let spec = QuerySpec {
            filters: vec![number("id", Operator::AnyOf, &["1, 2 ,3"])],
            ..Default::default()
        };
        assert!(spec.compile("t").unwrap().contains("\"id\" IN (1, 2, 3)"));

        let spec = QuerySpec {
            filters: vec![text("level", Operator::AnyOf, &["ERROR,WARN"])],
            ..Default::default()
        };
        assert!(
            spec.compile("t")
                .unwrap()
                .contains("\"level\" IN ('ERROR', 'WARN')")
        );
    }

    #[test]
    fn a_limit_is_always_present_and_never_zero() {
        let spec = QuerySpec {
            limit: 0,
            ..Default::default()
        };
        assert!(spec.compile("t").unwrap().ends_with("LIMIT 1"));
    }

    // ── Vocabulary ──────────────────────────────────────────────────────────

    #[test]
    fn operators_are_offered_only_where_they_mean_something() {
        // Ordering on text, or substrings on numbers, are nonsense the menu
        // should not offer.
        assert!(Operator::GreaterThan.applies_to(ColumnType::Integer));
        assert!(!Operator::GreaterThan.applies_to(ColumnType::Text));
        assert!(Operator::Contains.applies_to(ColumnType::Text));
        assert!(!Operator::Contains.applies_to(ColumnType::Float));
        // Equality and emptiness apply to anything.
        assert!(Operator::Equals.applies_to(ColumnType::Text));
        assert!(Operator::IsEmpty.applies_to(ColumnType::Timestamp));
    }

    #[test]
    fn an_aggregate_names_its_own_output_column() {
        assert_eq!(
            Aggregate {
                function: AggregateFn::Count,
                field: String::new()
            }
            .output_name(),
            "count"
        );
        assert_eq!(
            Aggregate {
                function: AggregateFn::Sum,
                field: "total amount".into()
            }
            .output_name(),
            "sum_total_amount"
        );
    }

    #[test]
    fn the_summary_reads_the_query_back_as_words() {
        // The collapsed head has to keep the question visible, so this is
        // intent rather than syntax.
        let spec = QuerySpec {
            filters: vec![text("level", Operator::Equals, &["error"])],
            group_by: vec!["service".to_string()],
            sort: vec![Sort {
                field: "count".into(),
                descending: true,
            }],
            ..Default::default()
        };
        assert_eq!(
            spec.summary(),
            "level is error · by service · sorted by count descending"
        );
        assert_eq!(QuerySpec::default().summary(), "everything");
    }

    #[test]
    fn a_spec_round_trips_through_serialization() {
        // The builder's state crosses the render-node boundary.
        let spec = QuerySpec {
            filters: vec![number("id", Operator::Between, &["1", "9"])],
            combine: Combine::Any,
            aggregates: vec![Aggregate {
                function: AggregateFn::Average,
                field: "x".into(),
            }],
            sort: vec![Sort {
                field: "id".into(),
                descending: true,
            }],
            group_by: vec!["g".into()],
            limit: 25,
        };
        let back: QuerySpec = serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
        assert_eq!(back, spec);
    }
}
