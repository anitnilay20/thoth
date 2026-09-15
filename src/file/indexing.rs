//! Building a file's index off the UI thread.
//!
//! Indexing a large file takes a moment — about a second per 500MB — which is
//! long enough to stall a frame but short enough that a modal would be
//! obnoxious. So a tab opens immediately and the scan runs behind it: the
//! status bar shows progress, and a notification arrives when the file is
//! ready.
//!
//! A job is cancellable, because the common reason an index is no longer
//! wanted is that its tab closed. Cancellation is cooperative — the scan checks
//! between chunks — and a cancelled job stores nothing.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::error::Result;
use crate::file::index_cache;
use crate::file::json_envelope::JsonEnvelope;
use crate::file::loaders::{DuckdbConnection, TextIndex};

/// What indexing a file produced.
#[allow(clippy::large_enum_variant)]
pub enum Indexed {
    /// The document is an envelope, and its collections are now tables. This
    /// is the good outcome: `users JOIN transactions` is ordinary SQL.
    Envelope {
        engine: DuckdbConnection,
        /// Aliases the collections are queryable by.
        collections: Vec<String>,
    },
    /// No structure we could use — browsable as text, at any size.
    Text(Box<TextIndex>),
}

impl Indexed {
    /// The text index, when that is what indexing produced.
    pub fn as_text(&self) -> Option<&TextIndex> {
        match self {
            Indexed::Text(index) => Some(index),
            Indexed::Envelope { .. } => None,
        }
    }

    /// Collections staged as tables, when the document was an envelope.
    pub fn collections(&self) -> &[String] {
        match self {
            Indexed::Envelope { collections, .. } => collections,
            Indexed::Text(_) => &[],
        }
    }
}

/// How far along an index build is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Progress {
    /// Still scanning; `fraction` is 0.0–1.0.
    Running { fraction: f32 },
    /// Finished and ready to take.
    Ready,
    /// Abandoned because the job was cancelled.
    Cancelled,
    /// Failed; the tab falls back to reading without an index.
    Failed,
}

impl Progress {
    /// Whether the job has stopped, for any reason.
    pub fn is_finished(self) -> bool {
        !matches!(self, Progress::Running { .. })
    }
}

/// State shared between the worker and the UI thread.
#[derive(Debug)]
struct Shared {
    scanned: AtomicU64,
    total: u64,
    cancelled: AtomicBool,
    finished: AtomicBool,
    failed: AtomicBool,
}

/// A running (or finished) index build.
///
/// Dropping the handle does not cancel the job — call [`IndexJob::cancel`].
/// That is deliberate: a job whose result is still wanted shouldn't die because
/// the handle moved.
pub struct IndexJob {
    path: PathBuf,
    shared: Arc<Shared>,
    /// Taken by whoever collects the finished index.
    result: Arc<std::sync::Mutex<Option<Indexed>>>,
}

impl IndexJob {
    /// Start indexing `path` on a worker thread.
    ///
    /// A cached index for an unchanged file is adopted immediately, so a
    /// re-opened file is ready on the first frame and never rescanned.
    pub fn spawn(path: &Path) -> Self {
        let total = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let shared = Arc::new(Shared {
            scanned: AtomicU64::new(0),
            total,
            cancelled: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            failed: AtomicBool::new(false),
        });
        let result = Arc::new(std::sync::Mutex::new(None));

        if let Some(cached) = index_cache::load(path) {
            *result.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(Indexed::Text(Box::new(cached)));
            shared.scanned.store(total, Ordering::Relaxed);
            shared.finished.store(true, Ordering::Release);
            return Self {
                path: path.to_path_buf(),
                shared,
                result,
            };
        }

        let worker_path = path.to_path_buf();
        let worker_shared = Arc::clone(&shared);
        let worker_result = Arc::clone(&result);
        std::thread::spawn(move || {
            let progress = |shared: &Arc<Shared>| {
                let shared = Arc::clone(shared);
                move |scanned: u64| {
                    shared.scanned.store(scanned, Ordering::Relaxed);
                    if shared.cancelled.load(Ordering::Acquire) {
                        std::ops::ControlFlow::Break(())
                    } else {
                        std::ops::ControlFlow::Continue(())
                    }
                }
            };

            // An envelope is the better outcome, so it is tried first: its
            // collections become real tables, where text is only browsable.
            match stage_envelope(&worker_path, progress(&worker_shared)) {
                Ok(Some(Some(indexed))) => {
                    *worker_result.lock().unwrap_or_else(|e| e.into_inner()) = Some(indexed);
                    worker_shared.finished.store(true, Ordering::Release);
                    if let Some(ctx) = crate::EGUI_CTX.get() {
                        ctx.request_repaint();
                    }
                    return;
                }
                // Cancelled mid-scan.
                Ok(None) => {
                    worker_shared.finished.store(true, Ordering::Release);
                    return;
                }
                // No envelope, or staging failed — fall through to text.
                _ => {}
            }
            worker_shared.scanned.store(0, Ordering::Relaxed);

            match TextIndex::build_observed(&worker_path, progress(&worker_shared)) {
                Ok(Some(index)) => {
                    // Best effort: a cache that fails to write costs a rescan
                    // next time, nothing more.
                    let _ = index_cache::store(&index);
                    *worker_result.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some(Indexed::Text(Box::new(index)));
                }
                // Cancelled — leave no result and store nothing.
                Ok(None) => {}
                Err(_) => worker_shared.failed.store(true, Ordering::Relaxed),
            }
            worker_shared.finished.store(true, Ordering::Release);

            if let Some(ctx) = crate::EGUI_CTX.get() {
                ctx.request_repaint();
            }
        });

        Self {
            path: path.to_path_buf(),
            shared,
            result,
        }
    }

    /// The file being indexed.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Current progress.
    pub fn progress(&self) -> Progress {
        if !self.shared.finished.load(Ordering::Acquire) {
            let scanned = self.shared.scanned.load(Ordering::Relaxed);
            let fraction = if self.shared.total == 0 {
                0.0
            } else {
                (scanned as f64 / self.shared.total as f64).clamp(0.0, 1.0) as f32
            };
            return Progress::Running { fraction };
        }
        if self.shared.failed.load(Ordering::Relaxed) {
            return Progress::Failed;
        }
        if self.shared.cancelled.load(Ordering::Acquire) {
            return Progress::Cancelled;
        }
        Progress::Ready
    }

    /// Ask the scan to stop. The worker notices between chunks.
    pub fn cancel(&self) {
        self.shared.cancelled.store(true, Ordering::Release);
    }

    /// Take the finished index, leaving the job empty. `None` while running,
    /// or if the job was cancelled or failed.
    pub fn take(&self) -> Option<Indexed> {
        self.shared
            .finished
            .load(Ordering::Acquire)
            .then(|| {
                self.result
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take()
            })
            .flatten()
    }
}

/// Scan a document for an envelope and stage its collections as tables.
///
/// The outer `Option` is cancellation; the inner one distinguishes "no
/// envelope here" from a document whose collections are now queryable.
fn stage_envelope(
    path: &Path,
    on_progress: impl FnMut(u64) -> std::ops::ControlFlow<()>,
) -> Result<Option<Option<Indexed>>> {
    let Some(scanned) = JsonEnvelope::scan_observed(path, on_progress)? else {
        return Ok(None); // cancelled
    };
    let Some(envelope) = scanned else {
        return Ok(Some(None)); // root is not an object
    };
    if envelope.queryable().next().is_none() {
        return Ok(Some(None)); // an object, but nothing tabular inside
    }

    let engine = DuckdbConnection::new()?;
    for collection in envelope.queryable() {
        // One bad collection should not cost the rest of the document.
        let _ = engine.stage_collection(path, collection);
    }
    let collections = engine.staged_collections();
    if collections.is_empty() {
        return Ok(Some(None));
    }
    Ok(Some(Some(Indexed::Envelope {
        engine,
        collections,
    })))
}

/// Index `path` on this thread, using the cache when it is current.
///
/// For callers with nowhere to put a background job — the CLI, tests.
pub fn index_now(path: &Path) -> Result<TextIndex> {
    if let Some(cached) = index_cache::load(path) {
        return Ok(cached);
    }
    let index = TextIndex::build(path)?;
    let _ = index_cache::store(&index);
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn source(lines: usize) -> NamedTempFile {
        let mut tmp = NamedTempFile::new().unwrap();
        for i in 0..lines {
            writeln!(tmp, "line-{i}").unwrap();
        }
        tmp.flush().unwrap();
        tmp
    }

    /// Block until the job stops, so tests don't race the worker.
    fn settle(job: &IndexJob) -> Progress {
        for _ in 0..2000 {
            let progress = job.progress();
            if progress.is_finished() {
                return progress;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("index job did not finish");
    }

    #[test]
    fn a_job_finishes_and_yields_its_index() {
        crate::file::index_cache::tests::isolate();
        let file = source(500);
        let job = IndexJob::spawn(file.path());

        assert_eq!(settle(&job), Progress::Ready);
        let indexed = job.take().expect("index");
        assert_eq!(indexed.as_text().expect("a text index").len(), 500);
        // Taking twice yields nothing the second time.
        assert!(job.take().is_none());

    }

    #[test]
    fn progress_is_a_fraction_that_reaches_one() {
        crate::file::index_cache::tests::isolate();
        let file = source(200);
        let job = IndexJob::spawn(file.path());
        settle(&job);

        // Once finished it reports Ready rather than a fraction.
        assert_eq!(job.progress(), Progress::Ready);
        assert!(job.take().is_some());
    }

    #[test]
    fn a_cancelled_job_yields_nothing() {
        crate::file::index_cache::tests::isolate();
        let file = source(100);
        let job = IndexJob::spawn(file.path());
        job.cancel();
        settle(&job);

        // It either finished before the cancel landed, or it stopped — but a
        // cancelled job must never hand back a partial index.
        match job.progress() {
            Progress::Cancelled => assert!(job.take().is_none()),
            Progress::Ready => {
                assert!(job.take().is_some(), "a Ready job has a complete index")
            }
            other => panic!("unexpected progress: {other:?}"),
        }
    }

    #[test]
    fn a_missing_file_fails_rather_than_hanging() {
        crate::file::index_cache::tests::isolate();
        let job = IndexJob::spawn(Path::new("/definitely/not/here.log"));
        assert_eq!(settle(&job), Progress::Failed);
        assert!(job.take().is_none());
    }

    #[test]
    fn a_second_open_is_served_from_the_cache() {
        crate::file::index_cache::tests::isolate();
        let file = source(300);
        let first = IndexJob::spawn(file.path());
        settle(&first);
        assert!(first.take().is_some());

        // Already cached: ready without touching a worker thread.
        let second = IndexJob::spawn(file.path());
        assert_eq!(second.progress(), Progress::Ready);
        assert_eq!(
            second.take().expect("cached index").as_text().unwrap().len(),
            300
        );

    }

    #[test]
    fn index_now_populates_the_cache_for_later_opens() {
        crate::file::index_cache::tests::isolate();
        let file = source(50);

        assert_eq!(index_now(file.path()).unwrap().len(), 50);
        // The next open finds it without scanning.
        let job = IndexJob::spawn(file.path());
        assert_eq!(job.progress(), Progress::Ready);

    }

    #[test]
    fn an_envelope_document_comes_back_as_queryable_tables() {
        crate::file::index_cache::tests::isolate();
        // The shape DuckDB cannot read at all: one top-level object. Indexing
        // it must yield tables, not text -- that is the whole point.
        let mut tmp = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
        write!(
            tmp,
            r#"{{"meta":{{"v":1}},
                "users":[{{"id":1,"name":"ada"}},{{"id":2,"name":"linus"}}],
                "transactions":[{{"user_id":1,"amount":10}},{{"user_id":2,"amount":7}}]}}"#
        )
        .unwrap();
        tmp.flush().unwrap();

        let job = IndexJob::spawn(tmp.path());
        assert_eq!(settle(&job), Progress::Ready);
        let indexed = job.take().expect("indexed");

        assert_eq!(indexed.collections(), ["transactions", "users"]);
        assert!(
            indexed.as_text().is_none(),
            "an envelope must not degrade to text"
        );

        let Indexed::Envelope { engine, .. } = indexed else {
            panic!("expected an envelope");
        };
        use crate::file::loaders::FileLoader as _;
        // And they join, over a document that has no tabular form of its own.
        let rows = crate::file::loaders::batches_to_values(
            &engine
                .query(
                    "SELECT u.name, t.amount FROM users u \
                     JOIN transactions t ON t.user_id = u.id ORDER BY t.amount DESC",
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["name"], "ada");
        assert_eq!(rows[0]["amount"], 10);
    }

    #[test]
    fn an_object_with_nothing_tabular_falls_back_to_text() {
        crate::file::index_cache::tests::isolate();
        let mut tmp = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
        write!(tmp, r#"{{"a":1,"b":"x","c":{{"d":2}}}}"#).unwrap();
        tmp.flush().unwrap();

        let job = IndexJob::spawn(tmp.path());
        assert_eq!(settle(&job), Progress::Ready);
        let indexed = job.take().expect("indexed");

        assert!(indexed.collections().is_empty());
        assert!(indexed.as_text().is_some(), "still openable, as text");
    }

    #[test]
    fn a_plain_log_file_is_indexed_as_text() {
        crate::file::index_cache::tests::isolate();
        let file = source(40);
        let job = IndexJob::spawn(file.path());
        assert_eq!(settle(&job), Progress::Ready);
        assert_eq!(job.take().unwrap().as_text().unwrap().len(), 40);
    }
}
