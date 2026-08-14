//! Interface-neutral Seshat operations.
//!
//! UI events and CLI commands are adapters around this module. Database
//! selection, connection testing, metadata access, and query execution belong
//! here so every entry point follows the same path.

use crate::db::{self, Engine, Profile, QueryResult, TableInfo};

pub(crate) fn test_connection(engine: Engine, profile: &Profile) -> Result<String, String> {
    db::adapter(engine).test_connection(profile)
}

pub(crate) fn list_databases(engine: Engine, profile: &Profile) -> Result<Vec<String>, String> {
    db::adapter(engine).list_databases(profile)
}

pub(crate) fn list_indices(engine: Engine, profile: &Profile) -> Result<Vec<TableInfo>, String> {
    db::adapter(engine).list_tables(profile, "_all")
}

pub(crate) fn run_query(
    engine: Engine,
    profile: &Profile,
    sql: &str,
) -> Result<QueryResult, String> {
    db::adapter(engine).run_query(profile, sql)
}
