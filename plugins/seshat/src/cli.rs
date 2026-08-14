//! Seshat's display-free commands, exported through `plugin-cli` WIT.

use serde_json::{json, Map, Value};
use thoth_plugin_sdk::cli::{
    CliArg, CliArgKind, CliInvocation, CliOutput, CliSchema, CliSubcommand,
};
use url::Url;

use crate::{
    db::{self, AuthMode, Engine, Profile},
    query::{self, Bound},
    service,
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
            "THOTH_SESHAT_PASSWORD=secret thoth seshat ping --connection-string 'postgres://user@localhost:5432/postgres'".into(),
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
                    "THOTH_SESHAT_PASSWORD=secret thoth seshat ping --connection-string 'postgres://user@localhost:5432/postgres'".into(),
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
                about: "Run a query and render the returned rows as a table".into(),
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
                    "THOTH_SESHAT_PASSWORD=secret thoth seshat query --connection-string 'mysql://root@localhost:3306/app' -q 'select * from users'".into(),
                ],
            },
        ],
    }
}

fn connection_args() -> Vec<CliArg> {
    vec![
        CliArg {
            id: "connection".into(),
            help: "Saved connection id (preferred) or unique display name".into(),
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
            "Connection URL; saved connections are preferred. Inline passwords are exposed in process arguments and shell history; omit the password and set THOTH_SESHAT_PASSWORD instead",
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
    match invocation.subcommand.as_str() {
        "ping" => {
            let detail = service::test_connection(engine, &profile)?;
            Ok(CliOutput::one(json!({
                "status": "ok",
                "engine": engine_name(engine),
                "detail": detail,
            })))
        }
        "indices" => {
            let records = if engine == Engine::Elasticsearch {
                service::list_indices(engine, &profile)?
                    .into_iter()
                    .map(|table| json!({ "index": table.name }))
                    .collect()
            } else {
                service::list_databases(engine, &profile)?
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
            let query = if engine == Engine::Elasticsearch {
                string(&invocation.values, "index")
                    .map(|index| format!("{index}\n{query}"))
                    .unwrap_or_else(|| query.to_string())
            } else {
                query.to_string()
            };
            let prepared = query::prepare(engine, &query, limit);
            if prepared.bound == Bound::Unavailable {
                return Err("query cannot be safely bounded; use a read query with an explicit result limit".into());
            }
            let result = service::run_query(engine, &profile, &prepared.sql)?;
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
    let parsed = Url::parse(url).map_err(|error| format!("invalid connection URL: {error}"))?;
    let scheme = parsed.scheme();
    let inferred = match scheme {
        "postgres" | "postgresql" => Engine::Postgres,
        "mysql" => Engine::Mysql,
        "http" | "https" | "elasticsearch" => Engine::Elasticsearch,
        _ => return Err(format!("unsupported connection scheme '{scheme}'")),
    };
    let engine = requested.map(parse_engine).transpose()?.unwrap_or(inferred);
    let defaults = db::adapter(engine).connection_defaults();
    let host = parsed
        .host_str()
        .ok_or_else(|| "connection URL has no host".to_string())?
        .trim_start_matches('[')
        .trim_end_matches(']');
    let port = parsed.port().unwrap_or(defaults.port);
    let decode = |value: &str| {
        percent_encoding::percent_decode_str(value)
            .decode_utf8()
            .map(|value| value.into_owned())
            .map_err(|error| format!("connection URL contains invalid UTF-8: {error}"))
    };
    let user = decode(parsed.username())?;
    let inline_password = parsed.password().map(decode).transpose()?;
    let password = inline_password
        .or_else(|| std::env::var("THOTH_SESHAT_PASSWORD").ok())
        .unwrap_or_default();
    let database = decode(parsed.path().trim_start_matches('/'))?;
    let auth = if engine == Engine::Elasticsearch && user.is_empty() && password.is_empty() {
        AuthMode::None
    } else {
        AuthMode::Password
    };
    Ok((
        engine,
        Profile {
            host: host.into(),
            port,
            database: if database.is_empty() {
                defaults.database.into()
            } else {
                database
            },
            user: if user.is_empty() {
                defaults.user.into()
            } else {
                user
            },
            password: password.clone(),
            tls: scheme == "https",
            auth,
        },
    ))
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
    use super::{parse_connection_url, schema};
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
    fn parses_percent_encoded_credentials_and_ipv6() {
        let (_, profile) =
            parse_connection_url("postgres://alice%40example:p%40ss@[::1]:5440/my%20db", None)
                .unwrap();
        assert_eq!(profile.user, "alice@example");
        assert_eq!(profile.password, "p@ss");
        assert_eq!(profile.host, "::1");
        assert_eq!(profile.port, 5440);
        assert_eq!(profile.database, "my db");
    }
}
