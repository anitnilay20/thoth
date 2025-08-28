use std::sync::Arc;
use std::thread;
use std::{path::PathBuf, sync::mpsc};

use memchr::memmem;
use rayon::prelude::*;

use crate::file::lazy_loader::{FileType, LazyJsonFile, load_file_auto};

#[derive(Default, Debug, Clone)]
pub struct Search {
    pub query: String,
    pub results: Vec<usize>,
    pub scanning: bool,
    pub match_case: bool,
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

        if self.query.is_empty() || file.is_none() {
            self.scanning = false;
            return;
        }

        let path = file.as_ref().unwrap();

        // Open lazily (auto-detect NDJSON / array JSON / single object)
        let Ok((_detected, store)) = load_file_auto(path) else {
            self.scanning = false;
            return;
        };

        // Move the store into an Arc so threads can share it immutably.
        let store = Arc::new(store);

        // Parallel scan
        let results = match parallel_scan(store, &self.query, self.match_case) {
            Ok(v) => v,
            Err(_e) => {
                // You can surface the error if you prefer.
                self.scanning = false;
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
) -> anyhow::Result<Vec<usize>> {
    let total = store.len();
    if total == 0 {
        return Ok(Vec::new());
    }

    // Prepare needle
    let mut needle = query.as_bytes().to_vec();
    let fold = !match_case;
    if fold {
        ascii_lower_in_place(&mut needle);
    }
    let needle = Arc::new(needle);

    let mut hits: Vec<usize> = (0..total)
        .into_par_iter()
        .filter_map(|i| {
            let mut hay = store.raw_slice(i).ok()?;
            if fold {
                ascii_lower_in_place(&mut hay);
            }
            if memmem::find(&hay, &needle).is_some() {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    hits.sort_unstable();
    Ok(hits)
}

/// Cheap ASCII-only lowercasing; good MVP for logs/JSON.
fn ascii_lower_in_place(b: &mut [u8]) {
    for ch in b {
        if ch.is_ascii_uppercase() {
            *ch = ch.to_ascii_lowercase();
        }
    }
}
