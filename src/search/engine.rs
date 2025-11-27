use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;
use std::thread;
use std::{path::PathBuf, sync::mpsc};

use memchr::memmem;
use rayon::prelude::*;
use serde_json::Value;

use super::results::{
    FieldComponent, MatchFragment, MatchPreview, MatchTarget, SearchHit, SearchResults,
};
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
    let lowered_query = if match_case {
        None
    } else {
        Some(query.to_lowercase())
    };

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
            let mut fragments = fragments;
            let query_for_fields = lowered_query.as_deref().unwrap_or(query);
            collect_field_matches(i, &original, query_for_fields, match_case, &mut fragments);
            ensure_root_highlight(&mut fragments, i);

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
            matched_text: None,
            text_range: None,
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

fn collect_field_matches(
    record_index: usize,
    bytes: &[u8],
    needle: &str,
    match_case: bool,
    fragments: &mut Vec<MatchFragment>,
) {
    if needle.is_empty() || fragments.len() >= MAX_FRAGMENTS_PER_RECORD {
        return;
    }

    let value: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => return,
    };
    let root_path = record_index.to_string();
    collect_value_matches(&value, &root_path, needle, match_case, fragments);
}

fn collect_value_matches(
    value: &Value,
    path: &str,
    needle: &str,
    match_case: bool,
    fragments: &mut Vec<MatchFragment>,
) {
    if fragments.len() >= MAX_FRAGMENTS_PER_RECORD {
        return;
    }

    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let key_path = format!("{}.{}", path, key);
                append_matches(
                    &key_path,
                    FieldComponent::Key,
                    key,
                    needle,
                    match_case,
                    fragments,
                );
                if fragments.len() >= MAX_FRAGMENTS_PER_RECORD {
                    return;
                }
                collect_value_matches(val, &key_path, needle, match_case, fragments);
                if fragments.len() >= MAX_FRAGMENTS_PER_RECORD {
                    return;
                }
            }
        }
        Value::Array(items) => {
            for (idx, val) in items.iter().enumerate() {
                let item_path = format!("{}[{}]", path, idx);
                collect_value_matches(val, &item_path, needle, match_case, fragments);
                if fragments.len() >= MAX_FRAGMENTS_PER_RECORD {
                    return;
                }
            }
        }
        Value::String(text) => {
            append_matches(
                path,
                FieldComponent::Value,
                text,
                needle,
                match_case,
                fragments,
            );
        }
        Value::Number(num) => {
            let text = num.to_string();
            append_matches(
                path,
                FieldComponent::Value,
                &text,
                needle,
                match_case,
                fragments,
            );
        }
        Value::Bool(flag) => {
            let text = if *flag { "true" } else { "false" };
            append_matches(
                path,
                FieldComponent::Value,
                text,
                needle,
                match_case,
                fragments,
            );
        }
        Value::Null => {
            append_matches(
                path,
                FieldComponent::Value,
                "null",
                needle,
                match_case,
                fragments,
            );
        }
    }
}

fn append_matches(
    path: &str,
    component: FieldComponent,
    text: &str,
    needle: &str,
    match_case: bool,
    fragments: &mut Vec<MatchFragment>,
) {
    if needle.len() > text.len() {
        return;
    }

    for range in find_match_ranges(text, needle, match_case) {
        let matched_text = text.get(range.clone()).map(|s| s.to_string());
        fragments.push(field_fragment(
            path,
            component,
            matched_text,
            Some(range.clone()),
        ));
        if fragments.len() >= MAX_FRAGMENTS_PER_RECORD {
            break;
        }
    }
}

fn field_fragment(
    path: &str,
    component: FieldComponent,
    matched_text: Option<String>,
    text_range: Option<Range<usize>>,
) -> MatchFragment {
    MatchFragment {
        fragment_id: 0,
        target: MatchTarget::JsonField { component },
        byte_range: 0..0,
        path: Some(Arc::<str>::from(path.to_string())),
        confidence: 1.0,
        matched_text,
        text_range: text_range.and_then(|range| {
            let start = u32::try_from(range.start).ok()?;
            let end = u32::try_from(range.end).ok()?;
            Some(start..end)
        }),
    }
}

fn ensure_root_highlight(fragments: &mut Vec<MatchFragment>, record_index: usize) {
    if fragments.is_empty() {
        return;
    }
    let root_path = record_index.to_string();
    let has_root = fragments
        .iter()
        .any(|fragment| fragment.path.as_deref() == Some(root_path.as_str()));
    if has_root {
        return;
    }
    fragments.push(MatchFragment {
        fragment_id: 0,
        target: MatchTarget::JsonField {
            component: FieldComponent::EntireRow,
        },
        byte_range: 0..0,
        path: Some(Arc::<str>::from(root_path)),
        confidence: 0.6,
        matched_text: None,
        text_range: None,
    });
}

fn find_match_ranges(haystack: &str, needle: &str, match_case: bool) -> Vec<Range<usize>> {
    if haystack.is_empty() || needle.is_empty() {
        return Vec::new();
    }

    let hay = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    if needle_bytes.len() > hay.len() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut idx = 0;
    while idx + needle_bytes.len() <= hay.len() {
        let mut matched = true;
        for (offset, &b) in needle_bytes.iter().enumerate() {
            let hay_b = hay[idx + offset];
            if match_case {
                if hay_b != b {
                    matched = false;
                    break;
                }
            } else if !hay_b.eq_ignore_ascii_case(&b) {
                matched = false;
                break;
            }
        }

        if matched {
            ranges.push(idx..idx + needle_bytes.len());
            idx += needle_bytes.len();
        } else {
            idx += 1;
        }
    }

    ranges
}
