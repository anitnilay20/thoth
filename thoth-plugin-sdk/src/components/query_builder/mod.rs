mod spec;
#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

pub use spec::{Aggregate, AggregateFn, Combine, Filter, Operator, QueryError, QuerySpec, Sort};

use crate::components::ColumnType;

/// A column the builder can be pointed at.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct QueryField {
    /// Column name, as it appears in the data.
    pub name: String,
    /// What it holds, which decides the operators offered and how a typed
    /// value is written into SQL.
    #[builder(default)]
    #[serde(default)]
    pub column_type: ColumnType,
}

/// What the builder says about the last run.
///
/// The distinction is the point: a report is a quiet figure in the head, and a
/// failure is a red line the user has to be able to read — including the part
/// an engine puts on the lines after the first, which is usually where it says
/// which column it could not find (#53).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "text", rename_all = "snake_case")]
pub enum QueryStatus {
    /// What the run returned, e.g. `"4,812 rows · 24 ms"`.
    Report(String),
    /// Why it did not run, or did not finish — an engine's message verbatim.
    Failure(String),
}

impl QueryStatus {
    /// The one line there is room for: an engine's message is several, and the
    /// first is the one that names what went wrong.
    pub fn headline(&self) -> &str {
        let text = match self {
            QueryStatus::Report(text) | QueryStatus::Failure(text) => text.as_str(),
        };
        text.lines().next().unwrap_or("").trim()
    }

    /// Everything after the headline — the candidate columns, the offending
    /// line — or `None` when the message was one line to begin with.
    pub fn detail(&self) -> Option<&str> {
        let text = match self {
            QueryStatus::Report(text) | QueryStatus::Failure(text) => text.as_str(),
        };
        let rest = text.split_once('\n')?.1.trim();
        (!rest.is_empty()).then_some(rest)
    }

    /// Whether this is something that went wrong.
    pub fn is_failure(&self) -> bool {
        matches!(self, QueryStatus::Failure(_))
    }
}

/// The design's query builder: four lanes that compile to one statement.
///
/// The lanes are the query — there is no second copy of it to fall out of
/// step, and the SQL box shows what they mean rather than being editable.
/// Collapsing the lanes leaves the head, which reads the question back in
/// words, so hiding the builder never hides what is being asked.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct QueryBuilder {
    /// Stable id — the collapsed and SQL-shown states persist under it.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// The query as it stands.
    #[builder(default)]
    #[serde(default)]
    pub spec: QuerySpec,
    /// Columns available to the lanes.
    #[builder(default)]
    #[serde(default)]
    pub fields: Vec<QueryField>,
    /// The relation the SQL preview reads from.
    #[builder(default)]
    #[serde(default)]
    pub relation: String,
    /// What the last run produced or why it failed. An error from the
    /// compiler is shown in its place.
    #[serde(default)]
    pub status: Option<QueryStatus>,
    /// SQL the user typed, which then owns the query instead of the lanes.
    ///
    /// `None` — the ordinary case — means the lanes are the query and the SQL
    /// pane mirrors them. Typing in the pane fills this, and from then on it
    /// is what runs: the lanes cannot express `HAVING`, a percentile or a
    /// window function, and a builder that silently dropped them would be
    /// worse than one that admits it is no longer driving.
    ///
    /// While it is set the lanes are shown but not editable. The design's rule
    /// is that the query is never in two places at once; letting both be
    /// edited would be exactly that, and the last one touched would win
    /// invisibly. [`revert`](QueryBuilder::revert) hands it back to the lanes.
    #[serde(default, rename = "sql-override")]
    pub sql_override: Option<String>,
}

/// Whether `sql` groups — a `GROUP BY` that is SQL rather than part of a
/// string, a quoted identifier or a comment.
///
/// Only the *noun* in the status line rests on this, so it errs towards
/// "rows": a query that groups and is reported in rows is a smaller lie than
/// one that does not and is reported in groups.
fn mentions_group_by(sql: &str) -> bool {
    let mut bare = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // A literal or a quoted identifier: skip to its close, minding the
            // doubled quote that escapes one inside it.
            '\'' | '"' => {
                while let Some(end) = chars.next() {
                    if end == c {
                        if chars.peek() == Some(&c) {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
                bare.push(' ');
            }
            '-' if chars.peek() == Some(&'-') => {
                for rest in chars.by_ref() {
                    if rest == '\n' {
                        break;
                    }
                }
                bare.push(' ');
            }
            _ => bare.push(c.to_ascii_lowercase()),
        }
    }
    bare.split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|pair| pair[0] == "group" && pair[1] == "by")
}

impl QueryBuilder {
    /// Whether the query that would run returns *groups* rather than rows, so
    /// the status line can name what it actually produced (#55).
    pub fn returns_groups(&self) -> bool {
        match &self.sql_override {
            Some(sql) => mentions_group_by(sql),
            None => !self.spec.group_by.is_empty(),
        }
    }

    /// The SQL that should run: what the user typed, or what the lanes compile
    /// to.
    ///
    /// One place decides, so the pane, the Run button and the host can never
    /// disagree about which query is the query.
    pub fn sql(&self) -> Result<String, QueryError> {
        match &self.sql_override {
            Some(typed) => Ok(typed.clone()),
            None => self.spec.compile(&self.relation),
        }
    }

    /// Whether the user has taken the query over by typing SQL.
    pub fn is_overridden(&self) -> bool {
        self.sql_override.is_some()
    }

    /// Give the query back to the lanes, discarding the typed SQL.
    pub fn revert(&mut self) {
        self.sql_override = None;
    }
}

/// What the user did in a [`QueryBuilder`] this frame.
///
/// The query itself is [`QueryBuilder::spec`], edited in place — this only says
/// whether it moved, so a host can persist it without diffing.
#[cfg(feature = "egui")]
#[derive(Clone, Debug, Default)]
pub struct QueryBuilderOutput {
    /// An edit landed in one of the lanes, or in the SQL pane.
    pub changed: bool,
    /// The user asked to run the query.
    pub run: bool,
    /// The relation the query reads from may have changed meaning — the user
    /// typed SQL naming something else. Hosts that re-aim on relation changes
    /// can ignore this; it is here so they need not diff the text.
    pub sql_edited: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_shows_its_first_line_and_keeps_the_rest() {
        // DuckDB answers in several lines, and the one that names the column
        // is the first. The rest — the candidates, the offending line — is
        // worth keeping for the hover, not for the strip.
        let status = QueryStatus::Failure(
            "Binder Error: Referenced column \"usr\" not found!\n\
             Candidate bindings: \"user\"\n\
             \n\
             LINE 1: SELECT * FROM d WHERE \"usr\" = 1"
                .to_string(),
        );
        assert_eq!(
            status.headline(),
            "Binder Error: Referenced column \"usr\" not found!"
        );
        assert!(status.detail().unwrap().starts_with("Candidate bindings"));
        assert!(status.is_failure());
    }

    #[test]
    fn a_one_line_report_has_nothing_held_back() {
        let status = QueryStatus::Report("4,812 rows · 24 ms".to_string());
        assert_eq!(status.headline(), "4,812 rows · 24 ms");
        assert_eq!(status.detail(), None);
        assert!(!status.is_failure());
    }

    #[test]
    fn a_grouped_query_is_counted_in_groups() {
        // From the lanes, the group lane says so outright.
        let mut builder = QueryBuilder::default();
        assert!(!builder.returns_groups());
        builder.spec.group_by = vec!["service".to_string()];
        assert!(builder.returns_groups());
    }

    #[test]
    fn typed_sql_is_read_for_a_group_by_without_being_fooled_by_one_in_quotes() {
        let grouping = |sql: &str| {
            let mut builder = QueryBuilder::default();
            builder.sql_override = Some(sql.to_string());
            builder.returns_groups()
        };
        assert!(grouping("SELECT status, count(*) FROM d GROUP BY status"));
        assert!(grouping("select a\nfrom d\ngroup  by a"));
        // A string, a quoted identifier and a comment are not clauses.
        assert!(!grouping("SELECT * FROM d WHERE note = 'group by hand'"));
        assert!(!grouping(r#"SELECT "group by" FROM d"#));
        assert!(!grouping("SELECT * FROM d -- group by status\n"));
        assert!(!grouping("SELECT * FROM d"));
        // A doubled quote closes nothing, so what follows is still a literal.
        assert!(!grouping("SELECT * FROM d WHERE n = 'it''s group by'"));
    }
}
