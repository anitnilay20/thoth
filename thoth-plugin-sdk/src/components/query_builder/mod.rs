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
    /// What the last run produced, e.g. "4,812 rows · 24 ms", shown in the
    /// foot. An error from the compiler is shown in its place.
    #[serde(default)]
    pub status: Option<String>,
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

impl QueryBuilder {
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
