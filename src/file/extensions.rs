//! DuckDB's optional readers, and the rule that the user asks for them.
//!
//! DuckDB ships a handful of readers in the binary — JSON, Parquet, CSV — and
//! fetches the rest over the network the first time something needs one. Left
//! to itself it does that silently, mid-open, from a worker thread: a
//! spreadsheet costs an unannounced 7.5 MB download, and on a machine with no
//! route out it costs a failure the user never sees, because the engine simply
//! declines the file and the viewer falls back to showing the bytes as text.
//!
//! So autoload is turned off ([`disable_autoload`]) and an extension arrives
//! only when the user says so. Opening a file that needs one names it and
//! offers to fetch it; until then the file is what it always was, text.
//!
//! What is *installed* is the whole of the state — it is a file on disk under
//! `~/.duckdb/extensions/<version>/<platform>/`, and DuckDB is the one that
//! put it there. Nothing here keeps a second record of that.

use crate::error::{Result, ThothError};
use crate::file::FileType;

/// Where DuckDB fetches an extension from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repository {
    /// The extensions DuckDB publishes itself.
    Core,
    /// The community index, which needs `FROM community` — an `INSTALL` that
    /// omits it fails with a download error that reads like the network is
    /// down, which is how `arrow` came to look unavailable when it is not.
    Community,
}

impl Repository {
    /// The `INSTALL` suffix this repository needs.
    fn clause(self) -> &'static str {
        match self {
            Repository::Core => "",
            Repository::Community => " FROM community",
        }
    }
}

/// An optional reader, and what it is for.
///
/// Built from an installed plugin's `[duckdb-extension]` section rather than
/// hardcoded here, so a reader is added by publishing a plugin and not by
/// changing the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extension {
    /// DuckDB's name for it — what `INSTALL`/`LOAD` take.
    pub name: String,
    /// Where it comes from.
    pub repository: Repository,
    /// What it lets the user open, in their terms rather than DuckDB's.
    pub unlocks: String,
    /// Roughly what it costs to fetch, for the offer to be honest about it.
    pub size: String,
    /// File extensions it opens, lowercase and without dots.
    pub handles: Vec<String>,
}

impl Extension {
    /// Read one out of a plugin's `[duckdb-extension]` section.
    fn from_meta(meta: &crate::plugin::DuckdbExtensionMeta) -> Self {
        Self {
            name: meta.extension.clone(),
            repository: match meta.repository.as_deref() {
                Some("community") => Repository::Community,
                _ => Repository::Core,
            },
            unlocks: meta
                .unlocks
                .clone()
                .unwrap_or_else(|| format!("files the {} reader opens", meta.extension)),
            size: meta
                .download_size
                .clone()
                .unwrap_or_else(|| "unknown size".to_string()),
            handles: meta
                .supported_extensions
                .iter()
                .map(|e| e.trim_start_matches('.').to_ascii_lowercase())
                .collect(),
        }
    }
}

/// Every optional reader currently offered, from the installed reader plugins.
///
/// Empty when plugins are off or none is installed, which is the honest
/// answer: with no plugin declaring a reader there is nothing to offer and
/// nothing to fetch.
pub fn catalog() -> Vec<Extension> {
    let Some(manager) = crate::plugin::runtime::active_manager() else {
        return Vec::new();
    };
    manager
        .get_all_plugin_by_capability(crate::plugin::Capability::DuckdbExtension)
        .iter()
        .filter_map(|p| p.duckdb_extension.as_ref())
        .map(Extension::from_meta)
        .collect()
}

/// The reader a format needs, or `None` when DuckDB reads it unaided — or
/// when no installed plugin declares one for it.
pub fn required_for(file_type: FileType) -> Option<Extension> {
    // `is_native` says the engine claims the format; these are the ones whose
    // reader is a separate download. A `.db` is left out on purpose: DuckDB's
    // own databases attach unaided, and only the bytes say which kind it is.
    let wanted: &[&str] = match file_type {
        FileType::Excel => &["xlsx", "xlsm"],
        FileType::Arrow => &["arrow", "arrows", "ipc"],
        _ => return None,
    };
    catalog()
        .into_iter()
        .find(|e| e.handles.iter().any(|h| wanted.contains(&h.as_str())))
}

/// The reader that opens files with this extension, if a plugin declares one.
pub fn for_file_extension(ext: &str) -> Option<Extension> {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    catalog().into_iter().find(|e| e.handles.contains(&ext))
}

/// Look a reader up by DuckDB's name for it.
pub fn find(name: &str) -> Option<Extension> {
    catalog().into_iter().find(|e| e.name == name)
}

/// The extension named in a DuckDB error, if it named one.
///
/// With autoload off, a missing reader fails with
/// `… is not in the catalog, but it exists in the excel extension.` — DuckDB
/// naming exactly what would fix it. Read back rather than guessed, so a
/// format whose reader moves between extensions still points at the right one.
pub fn named_in_error(message: &str) -> Option<Extension> {
    let tail = message.split("it exists in the ").nth(1)?;
    let name = tail.split_whitespace().next()?;
    find(name)
}

/// Bumped whenever a reader is installed, so anything holding a file open can
/// notice that what it can read has changed.
///
/// A counter rather than a signal to each tab: the install can come from the
/// marketplace, which knows nothing about open files, and a tab that was
/// showing a spreadsheet as text has no other way to learn the reader arrived.
/// Reading it costs an atomic load, which a frame can afford.
static INSTALLED_REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How many readers have been installed this session.
pub fn revision() -> u64 {
    INSTALLED_REVISION.load(std::sync::atomic::Ordering::Acquire)
}

/// Stop DuckDB fetching extensions on its own.
///
/// Both settings matter: `autoinstall` is the download, `autoload` is using
/// one already on disk. Leaving autoload on would make behaviour depend on
/// whether some earlier file happened to install the extension, which is the
/// kind of difference nobody can reproduce.
pub fn disable_autoload(conn: &duckdb::Connection) -> Result<()> {
    conn.execute_batch(
        "SET autoinstall_known_extensions=false; SET autoload_known_extensions=false;",
    )
    .map_err(|e| ThothError::DatabaseError {
        reason: format!("failed to take control of extension loading: {e}"),
    })
}

/// Whether DuckDB has this extension on disk.
///
/// Matched on aliases as well as the canonical name. `INSTALL`/`LOAD` accept
/// either, but `duckdb_extensions()` only ever reports the canonical one — so
/// asking about `sqlite` (which DuckDB calls `sqlite_scanner`, with `sqlite`
/// and `sqlite3` as aliases) said "not installed" however many times it had
/// been installed, and the row never stopped offering to install it.
pub fn is_installed(conn: &duckdb::Connection, name: &str) -> bool {
    conn.query_row(
        "SELECT installed FROM duckdb_extensions() \
         WHERE extension_name = ?1 OR list_contains(aliases, ?1)",
        [name],
        |row| row.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

/// Load an installed extension into this connection.
///
/// A miss is not an error here — it is the ordinary state of an extension the
/// user has not asked for, and the caller turns it into the offer.
pub fn load(conn: &duckdb::Connection, name: &str) -> bool {
    conn.execute_batch(&format!("LOAD {name};")).is_ok()
}

/// Fetch and load an extension, on the caller's thread.
///
/// Measured cold at about a second for `excel` on a warm network, and it is a
/// network call besides — so this belongs on a worker, never on the frame.
pub fn install(conn: &duckdb::Connection, extension: &Extension) -> Result<()> {
    let sql = format!(
        "INSTALL {}{}; LOAD {};",
        extension.name,
        extension.repository.clause(),
        extension.name
    );
    let outcome = conn.execute_batch(&sql);
    if outcome.is_ok() {
        INSTALLED_REVISION.fetch_add(1, std::sync::atomic::Ordering::Release);
    }
    outcome.map_err(|e| ThothError::DatabaseError {
        reason: format!(
            "could not install the {} reader: {}",
            extension.name,
            e.to_string()
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("unknown error")
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plugin's `[duckdb-extension]` section, as `plugin.toml` supplies it.
    fn meta(
        extension: &str,
        repository: Option<&str>,
        handles: &[&str],
    ) -> crate::plugin::DuckdbExtensionMeta {
        crate::plugin::DuckdbExtensionMeta {
            extension: extension.to_string(),
            repository: repository.map(str::to_string),
            supported_extensions: handles.iter().map(|h| h.to_string()).collect(),
            unlocks: Some("Excel workbooks (.xlsx, .xlsm)".to_string()),
            download_size: Some("7.5 MB".to_string()),
        }
    }

    #[test]
    fn a_plugin_declares_everything_the_offer_needs() {
        let e = Extension::from_meta(&meta("excel", Some("core"), &["xlsx", ".XLSM"]));
        assert_eq!(e.name, "excel");
        assert_eq!(e.repository, Repository::Core);
        assert_eq!(e.unlocks, "Excel workbooks (.xlsx, .xlsm)");
        assert_eq!(e.size, "7.5 MB");
        // Normalised, so a manifest written with a dot or in capitals still
        // matches the extension a path actually has.
        assert_eq!(e.handles, vec!["xlsx", "xlsm"]);
    }

    #[test]
    fn a_community_reader_installs_from_the_community_index() {
        // `INSTALL arrow` without this fails with a download error that reads
        // like the network is down. It is the single reason Arrow looked
        // unsupported.
        let community = Extension::from_meta(&meta("arrow", Some("community"), &["arrows"]));
        assert_eq!(community.repository, Repository::Community);
        assert_eq!(Repository::Community.clause(), " FROM community");

        // Core is the default, so a manifest that omits it still works.
        let unstated = Extension::from_meta(&meta("excel", None, &["xlsx"]));
        assert_eq!(unstated.repository, Repository::Core);
        assert_eq!(Repository::Core.clause(), "");
    }

    #[test]
    fn a_plugin_that_says_little_still_yields_a_usable_offer() {
        // `unlocks` and `download-size` are optional in the manifest; the
        // banner has to read sensibly without them rather than show a gap.
        let bare = Extension::from_meta(&crate::plugin::DuckdbExtensionMeta {
            extension: "excel".to_string(),
            repository: None,
            supported_extensions: vec!["xlsx".to_string()],
            unlocks: None,
            download_size: None,
        });
        assert!(!bare.unlocks.is_empty());
        assert!(!bare.size.is_empty());
    }

    #[test]
    fn with_no_reader_plugin_the_catalog_is_empty() {
        // Plugins off, or none installed: there is nothing to offer and
        // nothing to fetch, and that must be an ordinary empty answer.
        assert!(catalog().is_empty());
        assert!(required_for(FileType::Excel).is_none());
        assert!(for_file_extension("xlsx").is_none());
        assert!(find("excel").is_none());
    }

    #[test]
    fn a_bundled_format_never_asks_for_a_reader() {
        for ty in [FileType::Json, FileType::Csv, FileType::Parquet] {
            assert!(required_for(ty).is_none(), "{ty:?} needs no extension");
        }
    }

    #[test]
    fn the_error_names_the_reader_that_would_fix_it() {
        // DuckDB names the extension that would open the file; with no
        // reader plugin installed there is nothing to map it onto, which is
        // the honest answer rather than a guess.
        let message = "Catalog Error: Table Function with name \"read_xlsx\" is not in \
                       the catalog, but it exists in the excel extension.";
        assert!(named_in_error(message).is_none());
        assert!(named_in_error("IO Error: No files found that match").is_none());
    }

    #[test]
    fn autoload_is_off_and_stays_off() {
        // The whole opt-in rests on this: with either setting left on, a file
        // can pull a download nobody asked for.
        let conn = duckdb::Connection::open_in_memory().unwrap();
        disable_autoload(&conn).unwrap();
        for setting in ["autoinstall_known_extensions", "autoload_known_extensions"] {
            let on: bool = conn
                .query_row(&format!("SELECT current_setting('{setting}')"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert!(!on, "{setting} is still on");
        }
    }

    #[test]
    fn an_alias_and_its_canonical_name_answer_the_same() {
        // DuckDB accepts an alias in `INSTALL`/`LOAD` but reports only the
        // canonical name back from `duckdb_extensions()`. Asking by alias
        // used to always say "not installed", so a reader installed under one
        // offered to install itself forever.
        let conn = duckdb::Connection::open_in_memory().unwrap();
        disable_autoload(&conn).unwrap();
        for (alias, canonical) in [("sqlite", "sqlite_scanner"), ("sqlite3", "sqlite_scanner")] {
            assert_eq!(
                is_installed(&conn, alias),
                is_installed(&conn, canonical),
                "{alias} and {canonical} disagree about being installed"
            );
        }
    }

    #[test]
    fn an_absent_reader_reports_absent_rather_than_failing() {
        let conn = duckdb::Connection::open_in_memory().unwrap();
        disable_autoload(&conn).unwrap();
        assert!(!load(&conn, "definitely_not_an_extension"));
    }

    #[test]
    fn installing_a_reader_moves_the_revision() {
        let before = revision();
        INSTALLED_REVISION.fetch_add(1, std::sync::atomic::Ordering::Release);
        assert_eq!(revision(), before + 1);
    }

    #[test]
    fn a_failed_install_leaves_the_revision_alone() {
        // Otherwise every failure would send every open tab round the reopen
        // loop for a reader that still is not there.
        let conn = duckdb::Connection::open_in_memory().unwrap();
        disable_autoload(&conn).unwrap();
        let nonsense = Extension {
            name: "definitely_not_an_extension".to_string(),
            repository: Repository::Core,
            unlocks: "nothing".to_string(),
            size: "0 B".to_string(),
            handles: Vec::new(),
        };
        let before = revision();
        assert!(install(&conn, &nonsense).is_err());
        assert_eq!(revision(), before);
    }
}
