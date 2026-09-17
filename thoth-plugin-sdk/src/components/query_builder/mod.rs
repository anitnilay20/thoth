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
}

/// What the user did in a [`QueryBuilder`] this frame.
///
/// The query itself is [`QueryBuilder::spec`], edited in place — this only says
/// whether it moved, so a host can persist it without diffing.
#[cfg(feature = "egui")]
#[derive(Clone, Debug, Default)]
pub struct QueryBuilderOutput {
    /// An edit landed in one of the lanes.
    pub changed: bool,
    /// The user asked to run the query.
    pub run: bool,
}
