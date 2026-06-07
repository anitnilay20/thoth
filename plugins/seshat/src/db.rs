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
#[derive(Clone, Debug, Serialize)]
pub struct TableInfo {
    pub schema: String,
    pub name: String,
    /// `"table"` or `"view"`.
    pub kind: String,
}

/// One column of a table (schema introspection — distinct from a result [`Column`]).
#[derive(Clone, Debug, Serialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    pub primary_key: bool,
}

/// The set of operations every database driver must support.
pub trait DbAdapter {
    /// Open a connection and verify it works; returns the server version string.
    fn test_connection(&self, p: &Profile) -> Result<String, String>;

    /// List databases available on the server.
    fn list_databases(&self, p: &Profile) -> Result<Vec<String>, String>;

    /// List schemas (namespaces) in the connected database.
    fn list_schemas(&self, p: &Profile) -> Result<Vec<String>, String>;

    /// List tables and views in `schema`.
    fn list_tables(&self, p: &Profile, schema: &str) -> Result<Vec<TableInfo>, String>;

    /// Describe the columns of `schema.table`.
    fn list_columns(
        &self,
        p: &Profile,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>, String>;

    /// Run an arbitrary SQL statement and return the result set.
    fn run_query(&self, p: &Profile, sql: &str) -> Result<QueryResult, String>;
}

/// The adapter for `engine`. Boxed so callers stay engine-agnostic.
pub fn adapter(engine: Engine) -> Box<dyn DbAdapter> {
    match engine {
        Engine::Postgres => Box::new(crate::pg::Postgres),
        // MySQL lands in Phase 2; fall back to Postgres so the type checks.
        Engine::Mysql => Box::new(crate::pg::Postgres),
    }
}
