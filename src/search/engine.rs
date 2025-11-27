use std::borrow::Cow;
use std::sync::Arc;
use std::thread;
use std::{path::PathBuf, sync::mpsc};

use memchr::memmem;
use rayon::prelude::*;

use super::results::{MatchFragment, MatchPreview, MatchTarget, SearchHit, SearchResults};
use crate::error::ThothError;
use crate::file::lazy_loader::{FileType, LazyJsonFile, load_file_auto};

const MAX_FRAGMENTS_PER_RECORD: usize = 64;
const PREVIEW_CONTEXT_BYTES: usize = 36;

#[derive(Default, Debug, Clone)]
pub struct Search {
    pub query: String,
    pub results: SearchResults,
    pub scanning: bool,
    pub match_case: bool,
    pub error: Option<ThothError>,
}

impl Search {
    /// Spawn in background and return a channel to receive the finished Search.
    pub fn start_scanning(
        &self,
        file: &Option<PathBuf>,
        file_type: &FileType,
    ) -> mpsc::Receiver<Search> {
        let (tx, rx) = mpsc::channel();
        let mut job = self.clone();
        let file = file.clone();
        let file_type = *file_type;

        // mark as scanning for the first UI update
        job.scanning = true;

        thread::spawn(move || {
            job.start_scanning_internal(&file, &file_type);
            let _ = tx.send(job); // send finished (scanning=false, results filled)
        });

        rx
    }

    /// Parallel substring scan over the file's records.
    /// Populates `self.results` with matching root indices, then sets `scanning = false`.
    pub fn start_scanning_internal(&mut self, file: &Option<PathBuf>, _file_type: &FileType) {
        self.scanning = true;
        self.results.clear();
        self.error = None;

        if self.query.is_empty() {
            self.scanning = false;
            return;
        }

        let Some(path) = file.as_ref() else {
            self.scanning = false;
            self.error = Some(ThothError::StateError {
                reason: "No file loaded".to_string(),
            });
            return;
        };

        // Open lazily (auto-detect NDJSON / array JSON / single object)
        let (_detected, store) = match load_file_auto(path) {
            Ok(result) => result,
            Err(e) => {
                self.scanning = false;
                self.error = Some(ThothError::SearchError {
                    query: self.query.clone(),
                    reason: format!("Failed to load file for search: {}", e),
                });
                return;
            }
        };

        // Move the store into an Arc so threads can share it immutably.
        let store = Arc::new(store);

        // Parallel scan
        let results = match parallel_scan(store, &self.query, self.match_case) {
            Ok(v) => v,
            Err(e) => {
                self.scanning = false;
                self.error = Some(ThothError::SearchError {
                    query: self.query.clone(),
                    reason: format!("Search operation failed: {}", e),
                });
                return;
            }
        };

        self.results = results;

        self.scanning = false;
    }
}

fn parallel_scan(
    store: Arc<LazyJsonFile>,
    query: &str,
    match_case: bool,
) -> crate::error::Result<SearchResults> {
    let total = store.len();
    if total == 0 {
        return Ok(SearchResults::default());
    }

    // Prepare needle
    let mut needle = query.as_bytes().to_vec();
    let fold = !match_case;
    if fold {
        ascii_lower_in_place(&mut needle);
    }
    let needle = Arc::new(needle);

    let needle_len = needle.len();
    let mut hits: Vec<SearchHit> = (0..total)
        .into_par_iter()
        .filter_map(|i| {
            let original = store.raw_slice(i).ok()?;
            let hay_cow: Cow<'_, [u8]> = if fold {
                let mut buf = original.clone();
                ascii_lower_in_place(&mut buf);
                Cow::Owned(buf)
            } else {
                Cow::Borrowed(original.as_slice())
            };
            let hay_slice = hay_cow.as_ref();

            let finder = memmem::Finder::new(needle.as_slice());
            let fragments = collect_fragments(&finder, hay_slice, needle_len)?;
            let preview = build_preview(&original, fragments.first().unwrap());

            Some(SearchHit {
                record_index: i,
                fragments,
                preview,
            })
        })
        .collect();

    hits.sort_unstable_by_key(|hit| hit.record_index);
    Ok(SearchResults::new(hits, total))
}

/// Cheap ASCII-only lowercasing; good MVP for logs/JSON.
fn ascii_lower_in_place(b: &mut [u8]) {
    for ch in b {
        if ch.is_ascii_uppercase() {
            *ch = ch.to_ascii_lowercase();
        }
    }
}

fn collect_fragments(
    finder: &memmem::Finder<'_>,
    hay: &[u8],
    needle_len: usize,
) -> Option<Vec<MatchFragment>> {
    let mut fragments = Vec::new();
    for start in finder.find_iter(hay) {
        let end = start + needle_len;
        let start_u32 = u32::try_from(start).ok()?;
        let end_u32 = u32::try_from(end).ok()?;
        fragments.push(MatchFragment {
            fragment_id: 0,
            target: MatchTarget::RawRecord,
            byte_range: start_u32..end_u32,
            path: None,
            confidence: 1.0,
        });

        if fragments.len() >= MAX_FRAGMENTS_PER_RECORD {
            break;
        }
    }

    if fragments.is_empty() {
        None
    } else {
        Some(fragments)
    }
}

fn build_preview(bytes: &[u8], fragment: &MatchFragment) -> Option<MatchPreview> {
    if bytes.is_empty() {
        return None;
    }

    let start = usize::try_from(fragment.byte_range.start).ok()?;
    let end = usize::try_from(fragment.byte_range.end).ok()?;
    if start >= end || end > bytes.len() {
        return None;
    }

    let before_start = start.saturating_sub(PREVIEW_CONTEXT_BYTES);
    let after_end = (end + PREVIEW_CONTEXT_BYTES).min(bytes.len());

    let mut before = sanitize_snippet(&bytes[before_start..start]);
    if before_start > 0 && !before.starts_with('…') {
        before = format!("…{}", before.trim_start());
    }

    let highlight = sanitize_snippet(&bytes[start..end]);

    let mut after = sanitize_snippet(&bytes[end..after_end]);
    if after_end < bytes.len() && !after.ends_with('…') {
        after = format!("{}…", after.trim_end());
    }

    Some(MatchPreview {
        before,
        highlight,
        after,
    })
}

fn sanitize_snippet(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut sanitized = String::with_capacity(text.len());
    let mut last_was_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                sanitized.push(' ');
                last_was_space = true;
            }
        } else {
            sanitized.push(ch);
            last_was_space = false;
        }
    }

    sanitized.trim().to_string()
}
