//! A byte-offset index over a file's lines, so any file can be shown as text
//! regardless of size.
//!
//! This is the floor under every other viewer. When a format has no reader, or
//! its structure is too large to model, the file is still openable: index the
//! line starts in one streaming pass, then seek to whichever lines are on
//! screen. Nothing but the index and the visible lines is ever resident.
//!
//! The index is one `u64` per line — a 500MB log with 360k lines costs under
//! 3MB. A file with pathologically long lines (a 500MB single-line JSON
//! document, say) would otherwise defeat that, so lines are split at
//! [`MAX_LINE`]: the index stays bounded and the viewer stays responsive, at
//! the cost of showing a long line as several.

use std::fs::File;
use std::io::{BufReader, Read};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use crate::error::{Result, ThothError};
use crate::platform::FileIO;

/// Longest run of bytes shown as a single line. Beyond this a synthetic break
/// is inserted, which bounds both the index and the width of any one row.
pub const MAX_LINE: usize = 4096;

/// Bytes read per scan iteration while indexing.
const SCAN_CHUNK: usize = 1 << 20;

/// Line starts for a file, and the reads that use them.
pub struct TextIndex {
    path: PathBuf,
    /// Held open so a read is one positional syscall rather than an open, a
    /// seek and a read. `read_at` takes `&self`, so reads stay concurrent.
    file: File,
    /// Byte offset of each line's first byte. Always starts with 0 for a
    /// non-empty file.
    line_starts: Vec<u64>,
    /// Total bytes, so the last line's end is known.
    size: u64,
}

impl TextIndex {
    /// Scan `path` once, recording where each line begins.
    pub fn build(path: &Path) -> Result<Self> {
        match Self::build_observed(path, |_| ControlFlow::Continue(())) {
            Ok(Some(index)) => Ok(index),
            // The observer never asks to stop, so cancellation is unreachable.
            Ok(None) => unreachable!("uncancellable build reported cancellation"),
            Err(e) => Err(e),
        }
    }

    /// Scan `path`, reporting bytes consumed and honouring a request to stop.
    ///
    /// `on_progress` is called every [`SCAN_CHUNK`] bytes with the running
    /// total; returning [`ControlFlow::Break`] abandons the scan and yields
    /// `Ok(None)`. This is what lets an index build run in the background and
    /// be cancelled when its tab closes.
    pub fn build_observed(
        path: &Path,
        on_progress: impl FnMut(u64) -> ControlFlow<()>,
    ) -> Result<Option<Self>> {
        Self::build_limited(path, u64::MAX, on_progress)
    }

    /// Index only the first `max_bytes`, for showing a file *now* while its
    /// real index builds behind it.
    ///
    /// The result describes a prefix and nothing more: reads past it return
    /// nothing rather than reading uncharted bytes. A megabyte is thousands of
    /// lines, which is far more than a screen, and costs a single read.
    pub fn preview(path: &Path, max_bytes: u64) -> Result<Self> {
        Self::build_limited(path, max_bytes, |_| ControlFlow::Continue(()))?.ok_or_else(|| {
            ThothError::FileReadError {
                path: path.to_path_buf(),
                reason: "preview was cancelled".to_string(),
            }
        })
    }

    fn build_limited(
        path: &Path,
        max_bytes: u64,
        mut on_progress: impl FnMut(u64) -> ControlFlow<()>,
    ) -> Result<Option<Self>> {
        let file = File::open(path).map_err(|e| ThothError::FileReadError {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;
        let size = file
            .metadata()
            .map_err(|e| ThothError::FileReadError {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })?
            .len();

        let mut reader = BufReader::new(file);
        let mut line_starts: Vec<u64> = Vec::new();
        let mut buf = vec![0u8; SCAN_CHUNK];
        let mut offset: u64 = 0;
        // Bytes since the last break, so an over-long line can be split.
        let mut run: usize = 0;

        if size > 0 {
            line_starts.push(0);
        }

        loop {
            let read = reader
                .read(&mut buf)
                .map_err(|e| ThothError::FileReadError {
                    path: path.to_path_buf(),
                    reason: e.to_string(),
                })?;
            if read == 0 {
                break;
            }

            let mut scanned = 0;
            while scanned < read {
                // Where the next newline falls within what is left of the chunk.
                let rest = &buf[scanned..read];
                let newline = memchr::memchr(b'\n', rest);
                // How far we may advance before a synthetic break is due.
                let until_break = MAX_LINE - run;

                match newline {
                    // A newline arrives before the line gets too long.
                    Some(at) if at < until_break => {
                        let start = offset + (scanned + at + 1) as u64;
                        if start < size {
                            line_starts.push(start);
                        }
                        scanned += at + 1;
                        run = 0;
                    }
                    // The line runs past MAX_LINE first: break it.
                    _ if rest.len() >= until_break => {
                        let start = offset + (scanned + until_break) as u64;
                        if start < size {
                            line_starts.push(start);
                        }
                        scanned += until_break;
                        run = 0;
                    }
                    // Chunk exhausted mid-line; carry the run into the next one.
                    _ => {
                        run += rest.len();
                        scanned = read;
                    }
                }
            }
            offset += read as u64;
            if offset >= max_bytes {
                break;
            }
            if on_progress(offset).is_break() {
                return Ok(None);
            }
        }

        // A full scan ends at EOF, so `offset == size`; a preview stops short,
        // and must describe only what it read.
        let size = size.min(offset);

        Ok(Some(Self {
            path: path.to_path_buf(),
            file: File::open(path).map_err(|e| ThothError::FileReadError {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })?,
            line_starts,
            size,
        }))
    }

    /// Rebuild from line starts already on disk, skipping the scan.
    pub(crate) fn from_parts(path: &Path, line_starts: Vec<u64>, size: u64) -> Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
            file: File::open(path).map_err(|e| ThothError::FileReadError {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })?,
            line_starts,
            size,
        })
    }

    /// The recorded line starts, for persisting the index.
    pub(crate) fn line_starts(&self) -> &[u64] {
        &self.line_starts
    }

    /// The file this index describes.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Number of indexed lines.
    pub fn len(&self) -> usize {
        self.line_starts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.line_starts.is_empty()
    }

    /// Total bytes in the file.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// The byte range of line `index`.
    fn range(&self, index: usize) -> Option<(u64, u64)> {
        let start = *self.line_starts.get(index)?;
        let end = self
            .line_starts
            .get(index + 1)
            .copied()
            .unwrap_or(self.size);
        Some((start, end))
    }

    /// Read lines `[start, start + count)`, seeking straight to them.
    ///
    /// Trailing newlines are trimmed; invalid UTF-8 is replaced rather than
    /// rejected, because a text fallback that refuses to open a file is no
    /// fallback at all.
    pub fn read(&self, start: usize, count: usize) -> Result<Vec<String>> {
        if count == 0 || start >= self.len() {
            return Ok(Vec::new());
        }
        let end_line = (start + count).min(self.len());
        let (from, _) = self.range(start).unwrap_or((0, 0));
        let (_, to) = self.range(end_line - 1).unwrap_or((from, from));
        if to <= from {
            return Ok(Vec::new());
        }

        let mut buf = vec![0u8; (to - from) as usize];
        let read = self
            .file
            .read_at(&mut buf, from)
            .map_err(|e| ThothError::FileReadError {
                path: self.path.clone(),
                reason: e.to_string(),
            })?;
        buf.truncate(read);

        Ok((start..end_line)
            .map(|line| {
                let (s, e) = self.range(line).unwrap_or((from, from));
                let (lo, hi) = (
                    ((s - from) as usize).min(buf.len()),
                    ((e - from) as usize).min(buf.len()),
                );
                let slice = &buf[lo..hi];
                let text = String::from_utf8_lossy(slice);
                text.trim_end_matches('\n')
                    .trim_end_matches('\r')
                    .to_string()
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write(contents: &[u8]) -> NamedTempFile {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(contents).unwrap();
        tmp.flush().unwrap();
        tmp
    }

    #[test]
    fn indexes_and_reads_lines() {
        let file = write(b"alpha\nbeta\ngamma\n");
        let index = TextIndex::build(file.path()).unwrap();

        assert_eq!(index.len(), 3);
        assert_eq!(index.read(0, 3).unwrap(), ["alpha", "beta", "gamma"]);
    }

    #[test]
    fn reads_a_window_from_the_middle() {
        let body: String = (0..1000).map(|i| format!("line-{i}\n")).collect();
        let file = write(body.as_bytes());
        let index = TextIndex::build(file.path()).unwrap();

        assert_eq!(index.len(), 1000);
        // Seeks straight there — no need to touch the preceding lines.
        assert_eq!(
            index.read(500, 3).unwrap(),
            ["line-500", "line-501", "line-502"]
        );
        assert_eq!(index.read(999, 5).unwrap(), ["line-999"]);
        assert!(index.read(1000, 5).unwrap().is_empty());
    }

    #[test]
    fn a_file_without_a_trailing_newline_keeps_its_last_line() {
        let file = write(b"one\ntwo");
        let index = TextIndex::build(file.path()).unwrap();

        assert_eq!(index.len(), 2);
        assert_eq!(index.read(0, 2).unwrap(), ["one", "two"]);
    }

    #[test]
    fn crlf_endings_are_trimmed() {
        let file = write(b"one\r\ntwo\r\n");
        let index = TextIndex::build(file.path()).unwrap();
        assert_eq!(index.read(0, 2).unwrap(), ["one", "two"]);
    }

    #[test]
    fn an_empty_file_has_no_lines() {
        let file = write(b"");
        let index = TextIndex::build(file.path()).unwrap();
        assert_eq!(index.len(), 0);
        assert!(index.is_empty());
        assert!(index.read(0, 10).unwrap().is_empty());
    }

    #[test]
    fn blank_lines_are_preserved() {
        let file = write(b"a\n\n\nb\n");
        let index = TextIndex::build(file.path()).unwrap();
        assert_eq!(index.len(), 4);
        assert_eq!(index.read(0, 4).unwrap(), ["a", "", "", "b"]);
    }

    #[test]
    fn a_preview_indexes_only_a_prefix() {
        let body: String = (0..200_000).map(|i| format!("line-{i}\n")).collect();
        let file = write(body.as_bytes());

        let preview = TextIndex::preview(file.path(), 64 * 1024).unwrap();
        let full = TextIndex::build(file.path()).unwrap();

        assert!(preview.len() > 100, "enough to fill a screen");
        assert!(preview.len() < full.len(), "and not the whole file");
        // What it does have reads correctly...
        assert_eq!(preview.read(0, 2).unwrap(), ["line-0", "line-1"]);
        // ...and it never reads past what it charted.
        assert!(preview.read(preview.len(), 10).unwrap().is_empty());
        assert!(preview.size() <= full.size());
    }

    #[test]
    fn a_preview_of_a_small_file_is_the_whole_file() {
        let file = write(b"a\nb\nc\n");
        let preview = TextIndex::preview(file.path(), 1 << 20).unwrap();
        assert_eq!(preview.read(0, 3).unwrap(), ["a", "b", "c"]);
    }

    #[test]
    fn a_very_long_line_is_split_so_the_index_stays_bounded() {
        // The shape that would otherwise defeat a line index: one enormous
        // line. It must still be openable, and the rows must stay narrow.
        let body = vec![b'x'; MAX_LINE * 3 + 10];
        let file = write(&body);
        let index = TextIndex::build(file.path()).unwrap();

        assert_eq!(index.len(), 4, "split at MAX_LINE boundaries");
        let lines = index.read(0, 4).unwrap();
        assert_eq!(lines[0].len(), MAX_LINE);
        assert_eq!(lines[3].len(), 10);
        // Nothing was lost in the split.
        assert_eq!(lines.concat().len(), body.len());
    }

    #[test]
    fn splitting_survives_a_line_spanning_scan_chunks() {
        // A run longer than the read buffer exercises the carry between chunks.
        let body = vec![b'y'; SCAN_CHUNK + MAX_LINE + 7];
        let file = write(&body);
        let index = TextIndex::build(file.path()).unwrap();

        let lines = index.read(0, index.len()).unwrap();
        assert_eq!(lines.concat().len(), body.len(), "no bytes dropped");
        assert!(lines.iter().all(|l| l.len() <= MAX_LINE));
    }

    #[test]
    fn invalid_utf8_still_opens() {
        // A text fallback that refuses a file is not a fallback.
        let file = write(b"ok\n\xff\xfe binary \xff\nlast\n");
        let index = TextIndex::build(file.path()).unwrap();

        assert_eq!(index.len(), 3);
        let lines = index.read(0, 3).unwrap();
        assert_eq!(lines[0], "ok");
        assert!(lines[1].contains("binary"));
        assert_eq!(lines[2], "last");
    }
}

#[cfg(test)]
mod real_file_tests {
    use super::*;
    use std::path::Path;

    /// Indexing the 500MB envelope file that motivated this fallback. Ignored
    /// by default because it depends on a local file.
    #[test]
    #[ignore = "requires ~/Downloads/data_2gb.json"]
    fn indexes_a_2gb_document() {
        let path = Path::new(concat!(env!("HOME"), "/Downloads/data_2gb.json"));
        if !path.exists() {
            return;
        }
        let started = std::time::Instant::now();
        let index = TextIndex::build(path).unwrap();
        println!("lines: {}", index.len());
        println!("build time: {:?}", started.elapsed());
        println!("index bytes: ~{}", index.len() * 8);

        let started = std::time::Instant::now();
        let lines = index.read(index.len() / 2, 5).unwrap();
        println!("mid-file window: {:?}", started.elapsed());
        println!("sample: {}", &lines[0][..lines[0].len().min(90)]);

        crate::file::index_cache::store(&index).unwrap();
        let started = std::time::Instant::now();
        let cached = crate::file::index_cache::load(path).expect("cache hit");
        println!("cached open: {:?}", started.elapsed());
        assert_eq!(cached.len(), index.len());
    }

    #[test]
    #[ignore = "requires ~/Downloads/data_500mb.json"]
    fn indexes_a_500mb_document() {
        let path = Path::new(concat!(env!("HOME"), "/Downloads/data_500mb.json"));
        if !path.exists() {
            return;
        }
        let started = std::time::Instant::now();
        let index = TextIndex::build(path).unwrap();
        let elapsed = started.elapsed();

        println!("lines: {}", index.len());
        println!("index bytes: ~{}", index.len() * 8);
        println!("build time: {elapsed:?}");

        // A window from deep in the file is a seek, not a scan.
        let started = std::time::Instant::now();
        let lines = index.read(index.len() / 2, 20).unwrap();
        println!("mid-file window: {:?}", started.elapsed());
        assert_eq!(lines.len(), 20);

        // And the second open should come from cache, not another scan.
        crate::file::index_cache::store(&index).unwrap();
        let started = std::time::Instant::now();
        let cached = crate::file::index_cache::load(path).expect("cache hit");
        println!("cached open: {:?}", started.elapsed());
        println!(
            "cache entry bytes: {}",
            crate::file::index_cache::size_on_disk()
        );
        assert_eq!(cached.len(), index.len());
    }
}
