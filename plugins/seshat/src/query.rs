//! Shared query preparation used by every Seshat interface.

use crate::db::Engine;

/// How the server-side result bound was obtained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bound {
    /// Seshat added a bound to the submitted query.
    Applied,
    /// The query already contained an explicit bound.
    Existing,
    /// The query cannot be safely bounded by Seshat.
    Unavailable,
}

/// A query after engine-specific preparation.
pub(crate) struct Prepared {
    pub sql: String,
    pub bound: Bound,
}

/// Apply the engine-specific server-side row bound, or classify why the query
/// is left unchanged. UI and CLI callers consume this same result instead of
/// interpreting the lower-level SQL/Elasticsearch rewriters independently.
pub(crate) fn prepare(engine: Engine, query: &str, limit: usize) -> Prepared {
    let applied = if engine == Engine::Elasticsearch {
        crate::es::cap(query, limit)
    } else {
        crate::sql::add_limit(query, limit)
    };
    if let Some(sql) = applied {
        return Prepared {
            sql,
            bound: Bound::Applied,
        };
    }

    let existing = if engine == Engine::Elasticsearch {
        crate::es::has_explicit_cap(query)
    } else {
        crate::sql::has_explicit_limit(query)
    };
    Prepared {
        sql: query.to_string(),
        bound: if existing {
            Bound::Existing
        } else {
            Bound::Unavailable
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{prepare, Bound};
    use crate::db::Engine;

    #[test]
    fn applies_a_bound_to_plain_selects() {
        let query = prepare(Engine::Postgres, "select * from logs", 25);
        assert_eq!(query.sql, "select * from logs LIMIT 25");
        assert_eq!(query.bound, Bound::Applied);
    }

    #[test]
    fn preserves_existing_bounds() {
        let query = prepare(Engine::Postgres, "SELECT id FROM cr_users LIMIT 10", 200);
        assert_eq!(query.sql, "SELECT id FROM cr_users LIMIT 10");
        assert_eq!(query.bound, Bound::Existing);
    }

    #[test]
    fn identifies_queries_that_cannot_be_bounded() {
        let query = prepare(Engine::Mysql, "show tables", 25);
        assert_eq!(query.sql, "show tables");
        assert_eq!(query.bound, Bound::Unavailable);
    }
}
