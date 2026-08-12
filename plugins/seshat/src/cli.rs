//! Seshat's display-free commands, exported through `plugin-cli` WIT.

use serde_json::{json, Map, Value};
use thoth_plugin_sdk::cli::{
    CliArg, CliArgKind, CliInvocation, CliOutput, CliSchema, CliSubcommand,
};

use crate::{
    db::{self, AuthMode, Engine, Profile},
    state::STATE,
};

pub(crate) fn schema() -> CliSchema {
    CliSchema {
        id: "seshat".into(),
        about: "Query PostgreSQL, MySQL, or Elasticsearch".into(),
        examples: vec![
            "thoth seshat connections".into(),
            "thoth seshat ping production".into(),
            "thoth seshat indices production".into(),
            "thoth seshat query production -q 'select * from logs'".into(),
            "thoth seshat ping --connection-string 'postgres://user:password@localhost:5432/postgres'".into(),
        ],
        subcommands: vec![
            CliSubcommand {
                name: "connections".into(),
                about: "List saved Seshat connections".into(),
                args: vec![],
                examples: vec!["thoth seshat connections".into()],
            },
            CliSubcommand {
                name: "ping".into(),
                about: "Test a database connection".into(),
                args: connection_args(),
                examples: vec![
                    "thoth seshat ping production".into(),
                    "thoth seshat ping --connection-string 'postgres://user:password@localhost:5432/postgres'".into(),
                ],
            },
            CliSubcommand {
                name: "indices".into(),
                about: "List databases or Elasticsearch indices".into(),
                args: connection_args(),
                examples: vec![
                    "thoth seshat indices production".into(),
                    "thoth seshat indices --connection-string 'http://localhost:9200'".into(),
                ],
            },
            CliSubcommand {
                name: "query".into(),
                about: "Run a query and emit one JSON object per row".into(),
                args: {
                    let mut args = connection_args();
                    args.extend([
                        option(
                            "index",
                            "index",
                            None,
                            "INDEX",
                            "Elasticsearch index",
                            false,
                        ),
                        option(
                            "query",
                            "query",
                            Some('q'),
                            "QUERY",
                            "SQL, ES|QL, or Query DSL",
                            true,
                        ),
                        option(
                            "limit",
                            "limit",
                            None,
                            "ROWS",
                            "Maximum returned rows",
                            false,
                        ),
                    ]);
                    args
                },
                examples: vec![
                    "thoth seshat query production -q 'select * from logs' --limit 100".into(),
                    "thoth seshat query elastic-local --index logs -q '{\"query\":{\"match_all\":{}}}'".into(),
                    "thoth seshat query --connection-string 'mysql://root:password@localhost:3306/app' -q 'show tables'".into(),
                ],
            },
        ],
    }
}

fn connection_args() -> Vec<CliArg> {
    vec![
        CliArg {
            id: "connection".into(),
            help: "Saved connection id or display name".into(),
            required: false,
            kind: CliArgKind::Positional {
                value_name: "CONNECTION".into(),
            },
        },
        option(
            "engine",
            "engine",
            None,
            "ENGINE",
            "postgres, mysql, or elasticsearch",
            false,
        ),
        option(
            "connection_string",
            "connection-string",
            None,
            "URL",
            "Connection URL; otherwise use a saved Seshat connection",
            false,
        ),
    ]
}

fn option(
    id: &str,
    long: &str,
    short: Option<char>,
    value_name: &str,
    help: &str,
    required: bool,
) -> CliArg {
    CliArg {
        id: id.into(),
        help: help.into(),
        required,
        kind: CliArgKind::Option {
            long: long.into(),
            short,
            value_name: value_name.into(),
        },
    }
}

pub(crate) fn run(invocation: CliInvocation) -> Result<CliOutput, String> {
    if invocation.subcommand == "connections" {
        let records = crate::state::saved_connections()
            .into_iter()
            .map(|connection| {
                json!({
                    "id": connection.id,
                    "name": connection.name,
                    "engine": engine_name(connection.engine),
                    "host": connection.host,
                    "port": connection.port,
                    "database": connection.database,
                })
            })
            .collect();
        return Ok(CliOutput { records });
    }
    let (engine, profile) = connection(&invocation.values)?;
    let adapter = db::adapter(engine);
    match invocation.subcommand.as_str() {
        "ping" => {
            let detail = adapter.test_connection(&profile)?;
            Ok(CliOutput::one(json!({
                "status": "ok",
                "engine": engine_name(engine),
                "detail": detail,
            })))
        }
        "indices" => {
            let records = if engine == Engine::Elasticsearch {
                adapter
                    .list_tables(&profile, "_all")?
                    .into_iter()
                    .map(|table| json!({ "index": table.name }))
                    .collect()
            } else {
                adapter
                    .list_databases(&profile)?
                    .into_iter()
                    .map(|database| json!({ "database": database }))
                    .collect()
            };
            Ok(CliOutput { records })
        }
        "query" => {
            let query = string(&invocation.values, "query")
                .ok_or_else(|| "query requires --query/-q".to_string())?;
            let limit = string(&invocation.values, "limit")
                .map(|value| value.parse::<usize>())
                .transpose()
                .map_err(|_| "--limit must be a positive integer".to_string())?
                .unwrap_or(100);
            if limit == 0 {
                return Err("--limit must be greater than zero".into());
            }
            let query = cap_query(engine, string(&invocation.values, "index"), query, limit);
            let result = adapter.run_query(&profile, &query)?;
            let names: Vec<String> = result
                .columns
                .into_iter()
                .map(|column| column.name)
                .collect();
            let records = result
                .rows
                .into_iter()
                .take(limit)
                .map(|row| {
                    let mut record = Map::new();
                    for (index, name) in names.iter().enumerate() {
                        record.insert(name.clone(), row.get(index).cloned().unwrap_or(Value::Null));
                    }
                    Value::Object(record)
                })
                .collect();
            Ok(CliOutput { records })
        }
        command => Err(format!("unsupported Seshat command '{command}'")),
    }
}

fn connection(values: &Map<String, Value>) -> Result<(Engine, Profile), String> {
    if let Some(url) = string(values, "connection_string") {
        return parse_connection_url(url, string(values, "engine"));
    }
    if let Some(name) = string(values, "connection") {
        let (engine, profile) = crate::state::saved_profile(name)?;
        if let Some(requested) = string(values, "engine") {
            let requested = parse_engine(requested)?;
            if requested != engine {
                return Err(format!(
                    "saved connection '{name}' uses {}, not {}",
                    engine_name(engine),
                    engine_name(requested)
                ));
            }
        }
        return Ok((engine, profile));
    }
    let requested = string(values, "engine").map(parse_engine).transpose()?;
    STATE.with(|state| {
        let engine = requested.unwrap_or_else(|| state.engine());
        let profile = state.query_profile();
        if profile.host.trim().is_empty() {
            Err("no saved Seshat connection; pass --connection-string".into())
        } else {
            Ok((engine, profile))
        }
    })
}

fn parse_connection_url(url: &str, requested: Option<&str>) -> Result<(Engine, Profile), String> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| "connection string must be a URL".to_string())?;
    let inferred = match scheme {
        "postgres" | "postgresql" => Engine::Postgres,
        "mysql" => Engine::Mysql,
        "http" | "https" | "elasticsearch" => Engine::Elasticsearch,
        _ => return Err(format!("unsupported connection scheme '{scheme}'")),
    };
    let engine = requested.map(parse_engine).transpose()?.unwrap_or(inferred);
    let (credentials, address) = rest.rsplit_once('@').unwrap_or(("", rest));
    let (user, password) = credentials.split_once(':').unwrap_or((credentials, ""));
    let (authority, database) = address.split_once('/').unwrap_or((address, ""));
    let defaults = db::adapter(engine).connection_defaults();
    let (host, port) = authority
        .rsplit_once(':')
        .map(|(host, port)| {
            port.parse::<u16>()
                .map(|port| (host, port))
                .map_err(|_| "connection URL has an invalid port".to_string())
        })
        .transpose()?
        .unwrap_or((authority, defaults.port));
    if host.is_empty() {
        return Err("connection URL has no host".into());
    }
    Ok((
        engine,
        Profile {
            host: host.into(),
            port,
            database: if database.is_empty() {
                defaults.database.into()
            } else {
                database.into()
            },
            user: if user.is_empty() {
                defaults.user.into()
            } else {
                user.into()
            },
            password: password.into(),
            tls: scheme == "https",
            auth: if engine == Engine::Elasticsearch && user.is_empty() && password.is_empty() {
                AuthMode::None
            } else {
                AuthMode::Password
            },
        },
    ))
}

fn cap_query(engine: Engine, index: Option<&str>, query: &str, limit: usize) -> String {
    if engine == Engine::Elasticsearch {
        let query = if let Some(index) = index {
            format!("{index}\n{query}")
        } else {
            query.to_string()
        };
        crate::es::cap(&query, limit).unwrap_or(query)
    } else {
        crate::sql::add_limit(query, limit).unwrap_or_else(|| query.to_string())
    }
}

fn string<'a>(values: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    values.get(key).and_then(Value::as_str)
}

fn parse_engine(value: &str) -> Result<Engine, String> {
    match value {
        "postgres" | "postgresql" => Ok(Engine::Postgres),
        "mysql" => Ok(Engine::Mysql),
        "elasticsearch" | "elastic" | "es" => Ok(Engine::Elasticsearch),
        _ => Err(format!("unsupported engine '{value}'")),
    }
}

fn engine_name(engine: Engine) -> &'static str {
    match engine {
        Engine::Postgres => "postgres",
        Engine::Mysql => "mysql",
        Engine::Elasticsearch => "elasticsearch",
    }
}

#[cfg(test)]
mod tests {
    use super::{cap_query, parse_connection_url, schema};
    use crate::db::Engine;

    #[test]
    fn schema_is_valid_and_exposes_expected_commands() {
        let schema = schema();
        schema.validate().unwrap();
        assert_eq!(schema.id, "seshat");
        assert_eq!(schema.subcommands.len(), 4);
    }

    #[test]
    fn parses_postgres_connection_url() {
        let (engine, profile) =
            parse_connection_url("postgres://alice:secret@db.local:5433/app", None).unwrap();
        assert_eq!(engine, Engine::Postgres);
        assert_eq!(profile.host, "db.local");
        assert_eq!(profile.port, 5433);
        assert_eq!(profile.database, "app");
        assert_eq!(profile.user, "alice");
        assert_eq!(profile.password, "secret");
    }

    #[test]
    fn caps_sql_queries() {
        assert_eq!(
            cap_query(Engine::Postgres, None, "select * from logs", 25),
            "select * from logs LIMIT 25"
        );
    }
}
