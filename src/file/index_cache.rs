//! On-disk cache for file indexes, so a file is scanned once and reopened
//! instantly afterwards.
//!
//! Indexes live beside Thoth's other data, under
//! `<config>/thoth/data/index/`, one file per source keyed by a hash of its
//! absolute path.
//!
//! ## Staleness
//!
//! A cached index describes a specific file. Size and mtime catch most edits,
//! but not all — a restore from backup or an in-place edit of the same length
//! can leave both unchanged. Each entry therefore also records a fingerprint of
//! the file's first and last [`FINGERPRINT_BYTES`], which is cheap to compute
//! and catches the cases mtime misses. Hashing the whole file would be
//! correct but would cost as much as rebuilding the index, defeating the point.
//!
//! ## Format
//!
//! A small header followed by the line offsets as little-endian `u64`s. Line
//! offsets are numbers, and 380k of them are 3MB raw against roughly 3MB of
//! JSON that then has to be parsed — the binary form is both smaller and
//! quicker, and the file is a cache, so a format change just invalidates it.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{Result, ThothError};
use crate::file::loaders::TextIndex;
use crate::platform::FileIO;

/// Identifies the on-disk format. Bumping it invalidates every entry.
const MAGIC: &[u8; 8] = b"THOTHIX1";

/// Bytes sampled from each end of a file for its fingerprint.
const FINGERPRINT_BYTES: usize = 64 * 1024;

/// What a cache entry records about the file it describes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    size: u64,
    mtime_secs: i64,
    fingerprint: [u8; 32],
}

impl Stamp {
    /// Take a file's current stamp.
    fn of(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(|e| read_error(path, e))?;
        let meta = file.metadata().map_err(|e| read_error(path, e))?;
        let size = meta.len();
        let mtime_secs = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or_default();

        // Head and tail only: enough to notice a rewrite, cheap enough to be
        // worth doing on every open.
        let mut hasher = Sha256::new();
        hasher.update(size.to_le_bytes());
        let span = FINGERPRINT_BYTES.min(size as usize);
        let mut buf = vec![0u8; span];
        if span > 0 {
            let read = file.read_at(&mut buf, 0).map_err(|e| read_error(path, e))?;
            hasher.update(&buf[..read]);
            let tail_at = size.saturating_sub(span as u64);
            let read = file
                .read_at(&mut buf, tail_at)
                .map_err(|e| read_error(path, e))?;
            hasher.update(&buf[..read]);
        }

        Ok(Self {
            size,
            mtime_secs,
            fingerprint: hasher.finalize().into(),
        })
    }

    fn write(&self, out: &mut impl Write) -> std::io::Result<()> {
        out.write_all(&self.size.to_le_bytes())?;
        out.write_all(&self.mtime_secs.to_le_bytes())?;
        out.write_all(&self.fingerprint)
    }

    fn read(input: &mut impl Read) -> std::io::Result<Self> {
        let mut u64_buf = [0u8; 8];
        input.read_exact(&mut u64_buf)?;
        let size = u64::from_le_bytes(u64_buf);
        input.read_exact(&mut u64_buf)?;
        let mtime_secs = i64::from_le_bytes(u64_buf);
        let mut fingerprint = [0u8; 32];
        input.read_exact(&mut fingerprint)?;
        Ok(Self {
            size,
            mtime_secs,
            fingerprint,
        })
    }
}

/// Overrides the cache location. Set by tests so they never touch — let alone
/// clear — the user's real cache, and usable operationally to relocate it.
pub const CACHE_DIR_ENV: &str = "THOTH_INDEX_CACHE_DIR";

/// A file's identity, as the envelope cache records it: `(size, mtime, hex
/// fingerprint)`. Shared with the DuckDB-backed cache so both judge staleness
/// the same way.
pub fn identity(path: &Path) -> Result<(u64, i64, String)> {
    let stamp = Stamp::of(path)?;
    Ok((
        stamp.size,
        stamp.mtime_secs,
        stamp.fingerprint.iter().map(|b| format!("{b:02x}")).collect(),
    ))
}

/// Where a document's collections are cached as a DuckDB database.
///
/// Storing them as tables rather than extracted JSON means a second open
/// attaches instead of re-parsing, and queries run against columnar storage
/// instead of re-reading text every time.
pub fn database_path(path: &Path) -> Result<PathBuf> {
    let entry = entry_path(path)?;
    Ok(entry.with_extension("duckdb"))
}

/// Directory holding cached indexes, created on first use.
pub fn cache_dir() -> Result<PathBuf> {
    let dir = match std::env::var_os(CACHE_DIR_ENV) {
        Some(dir) => PathBuf::from(dir),
        None => dirs::config_dir()
            .ok_or_else(|| ThothError::StateError {
                reason: "failed to locate config directory".to_string(),
            })?
            .join("thoth")
            .join("data")
            .join("index"),
    };
    std::fs::create_dir_all(&dir).map_err(|e| ThothError::StateError {
        reason: format!("failed to create index cache directory: {e}"),
    })?;
    Ok(dir)
}

/// Where `path`'s index is cached. Keyed by the absolute path, so two files
/// with the same name in different directories don't collide.
fn entry_path(path: &Path) -> Result<PathBuf> {
    let absolute = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let digest = Sha256::digest(absolute.to_string_lossy().as_bytes());
    Ok(cache_dir()?.join(format!("{:x}.idx", digest)))
}

/// Load `path`'s cached index, or `None` if absent or stale.
///
/// A cache is an optimisation, so any problem reading one is treated as a
/// miss rather than an error — the caller rebuilds.
pub fn load(path: &Path) -> Option<TextIndex> {
    let entry = entry_path(path).ok()?;
    let current = Stamp::of(path).ok()?;
    let file = File::open(&entry).ok()?;
    let mut input = BufReader::new(file);

    let mut magic = [0u8; 8];
    input.read_exact(&mut magic).ok()?;
    if &magic != MAGIC {
        return None;
    }
    if Stamp::read(&mut input).ok()? != current {
        return None; // the file changed under us
    }

    let mut count_buf = [0u8; 8];
    input.read_exact(&mut count_buf).ok()?;
    let count = u64::from_le_bytes(count_buf) as usize;

    let mut line_starts = Vec::with_capacity(count);
    let mut offset_buf = [0u8; 8];
    for _ in 0..count {
        input.read_exact(&mut offset_buf).ok()?;
        line_starts.push(u64::from_le_bytes(offset_buf));
    }

    touch(&entry);
    TextIndex::from_parts(path, line_starts, current.size).ok()
}

/// Cache `index` for the file it describes.
///
/// Written to a temporary file and renamed, so an interrupted write cannot
/// leave a half-index that would later be read as complete.
pub fn store(index: &TextIndex) -> Result<()> {
    let path = index.path();
    let entry = entry_path(path)?;
    let stamp = Stamp::of(path)?;
    let temporary = entry.with_extension("idx.partial");

    {
        let file = File::create(&temporary).map_err(|e| write_error(&temporary, e))?;
        let mut out = BufWriter::new(file);
        out.write_all(MAGIC).map_err(|e| write_error(&temporary, e))?;
        stamp.write(&mut out).map_err(|e| write_error(&temporary, e))?;

        let starts = index.line_starts();
        out.write_all(&(starts.len() as u64).to_le_bytes())
            .map_err(|e| write_error(&temporary, e))?;
        for offset in starts {
            out.write_all(&offset.to_le_bytes())
                .map_err(|e| write_error(&temporary, e))?;
        }
        out.flush().map_err(|e| write_error(&temporary, e))?;
    }

    std::fs::rename(&temporary, &entry).map_err(|e| write_error(&entry, e))
}

/// Mark an entry as just used, so eviction sees it as recent.
///
/// Access times cannot be relied on — many systems disable or defer them — so a
/// hit records itself by moving the entry's modification time forward.
pub fn touch(path: &Path) {
    if let Ok(file) = File::options().write(true).open(path) {
        let _ = file.set_modified(std::time::SystemTime::now());
    }
}

/// Total bytes held by cached indexes.
pub fn size_on_disk() -> u64 {
    let Ok(dir) = cache_dir() else {
        return 0;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

/// Drop least-recently-used entries until the cache fits `budget_bytes`.
///
/// `in_use` names entries that must survive regardless of age: a cache
/// database an open tab is reading from would take its tab down with it.
pub fn enforce_budget(budget_bytes: u64, in_use: &std::collections::HashSet<PathBuf>) {
    let Ok(dir) = cache_dir() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };

    let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = entries
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            // Ordered by modification, not access: atime is unreliable (and
            // often disabled) on the platforms this runs on, so entries record
            // their own use by being touched on a hit.
            let touched = meta.modified().ok()?;
            Some((e.path(), meta.len(), touched))
        })
        .collect();

    // Everything counts toward the budget; only unused entries can be dropped.
    let total_all: u64 = files.iter().map(|(_, len, _)| len).sum();
    files.retain(|(path, _, _)| !in_use.contains(path));
    if total_all <= budget_bytes {
        return;
    }

    let mut total = total_all;

    // Oldest access first — those are the ones worth losing.
    files.sort_by_key(|(_, _, at)| *at);
    for (path, len, _) in files {
        if total <= budget_bytes {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(len);
        }
    }
}

/// Remove every cached index.
pub fn clear() -> Result<()> {
    let dir = cache_dir()?;
    for entry in std::fs::read_dir(&dir)
        .map_err(|e| ThothError::StateError {
            reason: format!("failed to read index cache: {e}"),
        })?
        .flatten()
    {
        let _ = std::fs::remove_file(entry.path());
    }
    Ok(())
}

fn read_error(path: &Path, e: std::io::Error) -> ThothError {
    ThothError::FileReadError {
        path: path.to_path_buf(),
        reason: e.to_string(),
    }
}

fn write_error(path: &Path, e: std::io::Error) -> ThothError {
    ThothError::FileWriteError {
        path: path.to_path_buf(),
        reason: e.to_string(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    /// These tests share one scratch directory, and some of them act on all of
    /// it (`clear`, `enforce_budget`), so they take turns.
    static DIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    pub(crate) fn exclusive() -> std::sync::MutexGuard<'static, ()> {
        DIR_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Point the cache at a scratch directory for the whole test binary, so a
    /// test run can never disturb the user's cache.
    pub(crate) fn isolate() {
        use std::sync::OnceLock;
        static SCRATCH: OnceLock<tempfile::TempDir> = OnceLock::new();
        let dir = SCRATCH.get_or_init(|| tempfile::tempdir().expect("scratch cache dir"));
        // SAFETY: tests in this binary only ever set this to the same value.
        unsafe { std::env::set_var(CACHE_DIR_ENV, dir.path()) };
    }

    fn source(contents: &[u8]) -> NamedTempFile {
        use std::io::Write as _;
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(contents).unwrap();
        tmp.flush().unwrap();
        tmp
    }

    #[test]
    fn an_index_round_trips_through_the_cache() {
        let _guard = exclusive();
        isolate();
        let file = source(b"alpha\nbeta\ngamma\n");
        let built = TextIndex::build(file.path()).unwrap();
        store(&built).unwrap();

        let cached = load(file.path()).expect("cache hit");
        assert_eq!(cached.len(), built.len());
        assert_eq!(cached.read(0, 3).unwrap(), ["alpha", "beta", "gamma"]);

    }

    #[test]
    fn a_rewritten_file_of_the_same_length_misses() {
        let _guard = exclusive();
        isolate();
        // The case size and mtime can both miss: an in-place edit of identical
        // length. The fingerprint is what catches it.
        let file = source(b"aaaa\nbbbb\n");
        store(&TextIndex::build(file.path()).unwrap()).unwrap();
        assert!(load(file.path()).is_some());

        std::fs::write(file.path(), b"cccc\ndddd\n").unwrap();
        assert!(
            load(file.path()).is_none(),
            "a same-length rewrite must invalidate the cache"
        );

    }

    #[test]
    fn a_changed_length_misses() {
        let _guard = exclusive();
        isolate();
        let file = source(b"one\ntwo\n");
        store(&TextIndex::build(file.path()).unwrap()).unwrap();

        std::fs::write(file.path(), b"one\ntwo\nthree\n").unwrap();
        assert!(load(file.path()).is_none());

    }

    #[test]
    fn an_uncached_file_misses_without_erroring() {
        let _guard = exclusive();
        isolate();
        let file = source(b"x\n");
        assert!(load(file.path()).is_none());
    }

    #[test]
    fn a_corrupt_entry_is_a_miss_not_a_failure() {
        let _guard = exclusive();
        isolate();
        let file = source(b"x\ny\n");
        let entry = entry_path(file.path()).unwrap();
        store(&TextIndex::build(file.path()).unwrap()).unwrap();
        std::fs::write(&entry, b"not an index").unwrap();

        assert!(load(file.path()).is_none(), "garbage reads as a miss");
    }

    #[test]
    fn entries_are_keyed_per_file() {
        let _guard = exclusive();
        isolate();
        let a = source(b"a\n");
        let b = source(b"b\n");
        assert_ne!(
            entry_path(a.path()).unwrap(),
            entry_path(b.path()).unwrap()
        );
    }

    #[test]
    fn storing_leaves_no_partial_file_behind() {
        let _guard = exclusive();
        isolate();
        let file = source(b"q\nr\n");
        store(&TextIndex::build(file.path()).unwrap()).unwrap();
        let entry = entry_path(file.path()).unwrap();
        assert!(!entry.with_extension("idx.partial").exists());
    }

    #[test]
    fn the_budget_evicts_least_recently_used_entries() {
        let _guard = exclusive();
        isolate();
        let _ = clear();
        let dir = cache_dir().unwrap();
        // Three entries with explicit ages, so the ordering under test is the
        // eviction policy rather than the filesystem's clock resolution.
        let now = std::time::SystemTime::now();
        for (age_secs, name) in [(300u64, "a.idx"), (200, "b.idx"), (100, "c.idx")] {
            std::fs::write(dir.join(name), vec![0u8; 4096]).unwrap();
            let file = File::options().write(true).open(dir.join(name)).unwrap();
            file.set_modified(now - std::time::Duration::from_secs(age_secs))
                .unwrap();
        }
        assert!(size_on_disk() >= 12288);

        // Room for roughly one entry.
        enforce_budget(5000, &std::collections::HashSet::new());

        assert!(size_on_disk() <= 5000, "trimmed to the budget");
        assert!(dir.join("c.idx").exists(), "the newest entry survives");
        assert!(!dir.join("a.idx").exists(), "the oldest went first");
    }

    #[test]
    fn an_entry_in_use_is_never_evicted() {
        let _guard = exclusive();
        isolate();
        let _ = clear();
        let dir = cache_dir().unwrap();
        let now = std::time::SystemTime::now();
        for (age_secs, name) in [(300u64, "old.idx"), (100, "new.idx")] {
            std::fs::write(dir.join(name), vec![0u8; 8192]).unwrap();
            let file = File::options().write(true).open(dir.join(name)).unwrap();
            file.set_modified(now - std::time::Duration::from_secs(age_secs))
                .unwrap();
        }

        // The oldest is open in a tab, so age must not decide its fate --
        // evicting it would take that tab down.
        let in_use = std::collections::HashSet::from([dir.join("old.idx")]);
        enforce_budget(1, &in_use);

        assert!(dir.join("old.idx").exists(), "an open cache survives");
        assert!(!dir.join("new.idx").exists(), "the rest still goes");
    }

    #[test]
    fn touching_an_entry_saves_it_from_the_next_eviction() {
        let _guard = exclusive();
        isolate();
        let _ = clear();
        let dir = cache_dir().unwrap();
        let now = std::time::SystemTime::now();
        for (age_secs, name) in [(300u64, "stale.idx"), (100, "fresh.idx")] {
            std::fs::write(dir.join(name), vec![0u8; 8192]).unwrap();
            let file = File::options().write(true).open(dir.join(name)).unwrap();
            file.set_modified(now - std::time::Duration::from_secs(age_secs))
                .unwrap();
        }

        // Using the older entry makes it the newer one.
        touch(&dir.join("stale.idx"));
        enforce_budget(8192, &std::collections::HashSet::new());

        assert!(dir.join("stale.idx").exists(), "a used entry is kept");
        assert!(!dir.join("fresh.idx").exists());
    }

    #[test]
    fn a_cache_within_budget_is_left_alone() {
        let _guard = exclusive();
        isolate();
        let _ = clear();
        let dir = cache_dir().unwrap();
        std::fs::write(dir.join("small.idx"), vec![0u8; 512]).unwrap();

        enforce_budget(1024 * 1024, &std::collections::HashSet::new());
        assert!(dir.join("small.idx").exists());
    }
}
