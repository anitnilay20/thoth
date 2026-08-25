use std::path::Path;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::{
    cli::{Action, CliOutput},
    file::loaders::{FileLoader, duck_db::DuckdbConnection},
};

pub fn command() -> Command {
    Command::new("query-file")
              .visible_alias("q")
              .about("Query data from one or more files using SQL")
              .long_about(
                  "Query CSV, JSON, Parquet, or database files.\n\
                   \n\
                   Each file is registered under an alias that you reference in your SQL. \
                   Use the form `alias=path`. If you omit the alias, it defaults to the \
                   file's name without its extension.\n\
                   \n\
                   EXAMPLES:\n    \
                   # Alias defaults to the file stem (\"sales\")\n    \
                   thoth query-file -f sales.parquet \"SELECT * FROM sales\"\n\
                   \n    \
                   # Explicit alias\n    \
                   thoth query-file -f s=sales.parquet \"SELECT * FROM s LIMIT 10\"\n\
                   \n    \
                   # Multiple files in one query (join across formats)\n    \
                   thoth query-file -f a=a.parquet -f b=b.csv \\\n        \
                   \"SELECT * FROM a JOIN b USING(id)\"",
              )
              .arg(
                  Arg::new("file")
                      .short('f')
                      .long("file")
                      .value_name("ALIAS=PATH")
                      .help("A file to query, as `alias=path` (alias optional; defaults to filename stem). Repeat -f for multiple files.")
                      .required(true)
                      .action(ArgAction::Append)          // allow repeating -f
                      .value_parser(value_parser!(String)),
              )
              .arg(
                  Arg::new("sql")
                      .value_name("SQL")
                      .help("SQL query to run. Reference each file by its alias, e.g. \"SELECT * FROM sales\"")
                      .required(true)
                      .value_parser(value_parser!(String)),
              )
}

pub fn parse(matches: &ArgMatches) -> Action {
    Action::QueryFile(
        matches
            .get_one::<String>("file")
            .expect("file is required")
            .to_string(),
        matches
            .get_one::<String>("sql")
            .expect("sql is required")
            .to_string(),
    )
}

fn parse_file_arg(raw: &str) -> Result<(String, String), String> {
    if let Some((alias, path)) = raw.split_once('=') {
        if alias.is_empty() || path.is_empty() {
            return Err(format!("invalid -f value `{raw}`: expected ALIAS=PATH"));
        }
        Ok((alias.to_string(), path.to_string()))
    } else {
        // No alias given — derive it from the file stem.
        let stem = Path::new(raw)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("cannot derive alias from `{raw}`; use ALIAS=PATH"))?;
        Ok((stem.to_string(), raw.to_string()))
    }
}

pub fn action(raw: &str, sql: &str) -> CliOutput {
    let (alias, path) = parse_file_arg(&raw).expect("");
    let db = DuckdbConnection::new().unwrap();
    db.open(&path, &alias);
    let result = db.query(&sql).unwrap();

    CliOutput {
        exit_code: 0,
        stdout: super::table_output::print_arrow(result),
        stderr: "".to_string(),
    }
}
