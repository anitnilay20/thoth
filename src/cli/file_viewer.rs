use std::path::Path;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::{
    cli::{Action, CliOutput},
    file::loaders::{FileLoader, duck_db::DuckdbConnection, duck_db::alias_for},
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
            .get_many::<String>("file")
            .expect("file is required")
            .cloned()
            .collect(),
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
        let path = Path::new(raw);
        if path.file_stem().is_none() {
            return Err(format!("cannot derive alias from `{raw}`; use ALIAS=PATH"));
        }
        Ok((alias_for(path), raw.to_string()))
    }
}

/// Register every `-f` file on one connection, then run the SQL against it.
///
/// A single connection is what makes cross-format joins work: each file
/// becomes a view, so `a JOIN b` can span a Parquet file and a CSV.
pub fn action(files: &[String], sql: &str) -> CliOutput {
    let db = match DuckdbConnection::new() {
        Ok(db) => db,
        Err(e) => return failure(format!("failed to start the query engine: {e}")),
    };

    for raw in files {
        let (alias, path) = match parse_file_arg(raw) {
            Ok(parsed) => parsed,
            Err(message) => return failure(message),
        };
        if let Err(e) = db.open(&path, &alias) {
            return failure(format!("failed to open `{path}` as `{alias}`: {e}"));
        }
    }

    match db.query(sql) {
        Ok(result) => CliOutput {
            exit_code: 0,
            stdout: super::table_output::print_arrow(result),
            stderr: String::new(),
        },
        Err(e) => failure(e.to_string()),
    }
}

fn failure(message: String) -> CliOutput {
    CliOutput {
        exit_code: 1,
        stdout: String::new(),
        stderr: format!("{message}\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn csv(contents: &str) -> NamedTempFile {
        let mut tmp = tempfile::Builder::new().suffix(".csv").tempfile().unwrap();
        tmp.write_all(contents.as_bytes()).unwrap();
        tmp.flush().unwrap();
        tmp
    }

    #[test]
    fn explicit_alias_is_used_verbatim() {
        assert_eq!(
            parse_file_arg("s=sales.parquet").unwrap(),
            ("s".to_string(), "sales.parquet".to_string())
        );
    }

    #[test]
    fn bare_path_derives_a_sql_safe_alias() {
        let (alias, path) = parse_file_arg("/tmp/sales-2024.csv").unwrap();
        assert_eq!(alias, "sales_2024");
        assert_eq!(path, "/tmp/sales-2024.csv");
    }

    #[test]
    fn malformed_alias_is_rejected() {
        assert!(parse_file_arg("=sales.csv").is_err());
        assert!(parse_file_arg("s=").is_err());
    }

    #[test]
    fn query_runs_against_the_registered_alias() {
        let file = csv("name,age\nada,36\nlinus,54\n");
        let arg = format!("people={}", file.path().display());

        let out = action(&[arg], "SELECT name FROM people ORDER BY age");
        assert_eq!(out.exit_code, 0, "stderr: {}", out.stderr);
        assert!(out.stdout.contains("ada"));
        assert!(out.stdout.contains("(2 rows)"));
    }

    #[test]
    fn multiple_files_join_on_one_connection() {
        let a = csv("id,v\n1,x\n");
        let b = csv("id,w\n1,y\n");

        let out = action(
            &[
                format!("a={}", a.path().display()),
                format!("b={}", b.path().display()),
            ],
            "SELECT v, w FROM a JOIN b USING(id)",
        );
        assert_eq!(out.exit_code, 0, "stderr: {}", out.stderr);
        assert!(out.stdout.contains('x') && out.stdout.contains('y'));
    }

    #[test]
    fn a_bad_query_fails_instead_of_panicking() {
        let file = csv("name\nada\n");
        let arg = format!("people={}", file.path().display());

        let out = action(&[arg], "SELECT nope FROM people");
        assert_eq!(out.exit_code, 1);
        assert!(!out.stderr.is_empty());
    }

    #[test]
    fn a_missing_file_fails_instead_of_panicking() {
        let out = action(&["x=/definitely/not/here.csv".to_string()], "SELECT 1");
        assert_eq!(out.exit_code, 1);
        assert!(out.stderr.contains("not/here.csv"));
    }
}
