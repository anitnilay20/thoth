//! Engine-agnostic database driver abstraction. Every supported engine
//! implements [`DbAdapter`]; Postgres ([`crate::pg::Postgres`]) is the first.
//! MySQL follows in Phase 2.
//!
//! The model is intentionally stateless: each method opens a connection over
//! the host `tcp-client` shim, does its work, and closes. All methods are
//! blocking and are only ever invoked on the host's db-runtime worker thread
//! (never the UI thread), via the plugin's `query` export.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Which database engine a connection targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    #[default]
    Postgres,
    Mysql,
}

/// A connection target: one database server plus credentials.
#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    pub tls: bool,
}

/// A result column with its engine-specific type name.
#[derive(Clone, Debug, Serialize)]
pub struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
}

/// A tabular query result. `rows` are positional, aligned with `columns`.
#[derive(Clone, Debug, Serialize)]
pub struct QueryResult {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Value>>,
    /// The command tag (e.g. `SELECT 3`, `INSERT 0 1`) when the server sent one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

/// A table or view within a schema.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TableInfo {
    /// The owning database, when the row comes from a cross-database search
    /// (`None` for the schema tree, which is already scoped to one database).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    pub schema: String,
    pub name: String,
    /// `"table"` or `"view"`.
    pub kind: String,
}

/// One column of a table (schema introspection — distinct from a result [`Column`]).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    pub primary_key: bool,
    /// Part of a UNIQUE constraint (and not the primary key).
    #[serde(default)]
    pub unique: bool,
    /// `Some("referenced_table.referenced_column")` when this column is a foreign key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreign_key: Option<String>,
}

/// One index on a table (for the structure view's Indexes tab).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

/// Everything the structure view shows for one table: its columns (with
/// constraint flags), its indexes, an estimated row count, and its on-disk size.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TableDetail {
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    /// Estimated row count (from catalog statistics — cheap, not `COUNT(*)`).
    pub row_estimate: i64,
    /// Human-readable total size (e.g. `318 MB`), empty when unavailable.
    pub size: String,
}

/// Engine-specific defaults + placeholders used to seed the new-connection form.
pub struct ConnectionDefaults {
    /// Default port.
    pub port: u16,
    /// Default superuser name (e.g. `postgres`, `root`).
    pub user: &'static str,
    /// Database value to prefill (may be empty — MySQL has no default database).
    pub database: &'static str,
    /// Placeholder hint shown in the (empty) database field.
    pub database_placeholder: &'static str,
}

/// The set of operations every database driver must support.
pub trait DbAdapter {
    /// Open a connection and verify it works; returns the server version string.
    fn test_connection(&self, p: &Profile) -> Result<String, String>;

    /// Engine defaults for a fresh connection form (port, user, database, hint).
    fn connection_defaults(&self) -> ConnectionDefaults;

    /// List databases available on the server.
    fn list_databases(&self, p: &Profile) -> Result<Vec<String>, String>;

    /// List schemas (namespaces) in the connected database.
    fn list_schemas(&self, p: &Profile) -> Result<Vec<String>, String>;

    /// List tables and views in `schema`.
    fn list_tables(&self, p: &Profile, schema: &str) -> Result<Vec<TableInfo>, String>;

    /// Find tables/views in the connected database whose name matches `query`
    /// (case-insensitive substring), across all user schemas. Capped server-side.
    /// This is the schema browser's server-side filter.
    fn find_tables(&self, p: &Profile, query: &str) -> Result<Vec<TableInfo>, String>;

    /// Describe the columns of `schema.table`.
    fn list_columns(
        &self,
        p: &Profile,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>, String>;

    /// Full detail for the structure view: columns (with PK/UNIQUE/FK flags),
    /// indexes, an estimated row count, and on-disk size. Enrichments degrade
    /// gracefully — a failed sub-query yields empty/zero rather than an error.
    fn describe_table(&self, p: &Profile, schema: &str, table: &str)
        -> Result<TableDetail, String>;

    /// Run an arbitrary SQL statement and return the result set.
    fn run_query(&self, p: &Profile, sql: &str) -> Result<QueryResult, String>;
}

/// The adapter for `engine`. Boxed so callers stay engine-agnostic.
pub fn adapter(engine: Engine) -> Box<dyn DbAdapter> {
    match engine {
        Engine::Postgres => Box::new(crate::pg::Postgres),
        Engine::Mysql => Box::new(crate::mysql::Mysql),
    }
}

impl Engine {
    /// The engine-specific `EXPLAIN` statement that yields a machine-readable
    /// plan **with actual run-time stats** for `sql`. Postgres `ANALYZE`
    /// executes the statement, so callers must only run this on demand.
    pub fn explain_sql(&self, sql: &str) -> String {
        match self {
            Engine::Postgres => format!("EXPLAIN (ANALYZE, FORMAT JSON) {sql}"),
            // MySQL's ANALYZE only emits TREE format, so JSON stays estimate-only
            // (no actual run times — the plan renderer bars by cost instead).
            Engine::Mysql => format!("EXPLAIN FORMAT=JSON {sql}"),
        }
    }
}
