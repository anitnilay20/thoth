//! Embedded BM25 full-text search index over local files (issue #151).
//!
//! Provides BM25-ranked search, phrase queries (`"connection refused"`),
//! boolean operators (`AND`, `OR`, `-term`), and field-scoped terms
//! (`host:prod`) over NDJSON, JSON, CSV, or plain-text/log files, with
//! optional on-disk index caching validated by file size, mtime, and
//! content fingerprint.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config_dir;

/// One document stored in the index (one line or JSON record).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FtsDocument {
    pub doc_id: usize,
    pub line_number: usize,
    pub text: String,
    pub doc_len: usize,
}

/// One ranked search hit.
#[derive(Clone, Debug, PartialEq)]
pub struct FtsHit {
    pub doc_id: usize,
    pub line_number: usize,
    pub score: f64,
    pub text: String,
}

/// Persisted file fingerprint so stale or corrupt cache entries are safely ignored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileStamp {
    pub size: u64,
    pub mtime_nanos: u128,
    pub head_hash: u64,
}

/// Embedded inverted index with BM25 scoring.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FtsIndex {
    pub stamp: Option<FileStamp>,
    pub docs: Vec<FtsDocument>,
    pub avg_doc_len: f64,
    /// `term -> Vec<(doc_id, term_freq)>`
    pub postings: HashMap<String, Vec<(usize, u32)>>,
}

#[derive(Clone, Debug)]
enum QueryClause {
    Term { field: Option<String>, term: String },
    Phrase { field: Option<String>, phrase: String },
    NegatedTerm { field: Option<String>, term: String },
}

impl FtsIndex {
    /// Build an in-memory index from an iterator of `(line_number, text)` pairs.
    pub fn from_lines<I, S>(lines: I) -> Self
    where
        I: IntoIterator<Item = (usize, S)>,
        S: Into<String>,
    {
        let mut docs = Vec::new();
        let mut postings: HashMap<String, Vec<(usize, u32)>> = HashMap::new();
        let mut total_len = 0usize;

        for (line_number, raw) in lines {
            let text: String = raw.into();
            let doc_id = docs.len();
            let tokens = extract_index_tokens(&text);
            let doc_len = tokens.len();
            total_len += doc_len;

            let mut tf: HashMap<String, u32> = HashMap::new();
            for tok in tokens {
                *tf.entry(tok).or_insert(0) += 1;
            }
            for (tok, freq) in tf {
                postings.entry(tok).or_default().push((doc_id, freq));
            }

            docs.push(FtsDocument {
                doc_id,
                line_number,
                text,
                doc_len,
            });
        }

        let avg_doc_len = if docs.is_empty() {
            1.0
        } else {
            (total_len as f64 / docs.len() as f64).max(1.0)
        };

        Self {
            stamp: None,
            docs,
            avg_doc_len,
            postings,
        }
    }

    /// Build or load a cached index for `path`. Cache misses or corrupt cache
    /// files never fail a read — they transparently rebuild from `path`.
    pub fn open_or_build(path: &Path) -> Result<Self, String> {
        let stamp = file_stamp(path).ok();
        if let Some(ref st) = stamp {
            if let Some(cached) = try_load_cache(path, st) {
                return Ok(cached);
            }
        }

        let file = File::open(path).map_err(|e| format!("Failed to open {}: {e}", path.display()))?;
        let reader = BufReader::new(file);
        let mut lines = Vec::new();
        for (idx, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| format!("Failed reading {}: {e}", path.display()))?;
            if !line.trim().is_empty() {
                lines.push((idx + 1, line));
            }
        }

        let mut index = Self::from_lines(lines);
        index.stamp = stamp;
        let _ = try_save_cache(path, &index);
        Ok(index)
    }

    /// Search the index using BM25 ranking.
    ///
    /// Supports:
    ///   * bare terms (`connection refused`)
    ///   * `AND` / `OR` operators
    ///   * negated terms (`-debug`)
    ///   * quoted phrases (`"connection refused"`)
    ///   * field-scoped terms (`host:prod`, `level:error`)
    pub fn search(&self, query: &str, limit: usize) -> Vec<FtsHit> {
        let parsed = parse_query(query);
        if parsed.groups.is_empty() || self.docs.is_empty() {
            return Vec::new();
        }

        let n = self.docs.len() as f64;
        let k1 = 1.2;
        let b = 0.75;

        let mut scores: HashMap<usize, f64> = HashMap::new();

        // Each OR-group is evaluated independently; documents matching any
        // OR-group are scored and merged by max/sum score.
        for group in &parsed.groups {
            let mut candidates: Option<HashSet<usize>> = None;
            let mut group_scores: HashMap<usize, f64> = HashMap::new();

            for clause in group {
                match clause {
                    QueryClause::Term { field, term } => {
                        let key = match field {
                            Some(f) => format!("{f}:{term}"),
                            None => term.clone(),
                        };
                        let Some(list) = self.postings.get(&key) else {
                            candidates = Some(HashSet::new());
                            break;
                        };
                        let df = list.len() as f64;
                        let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();

                        let mut set = HashSet::with_capacity(list.len());
                        for &(doc_id, tf) in list {
                            set.insert(doc_id);
                            let dl = self.docs[doc_id].doc_len as f64;
                            let tf_f = tf as f64;
                            let bm25 = idf * (tf_f * (k1 + 1.0))
                                / (tf_f + k1 * (1.0 - b + b * (dl / self.avg_doc_len)));
                            *group_scores.entry(doc_id).or_insert(0.0) += bm25;
                        }
                        candidates = Some(match candidates {
                            Some(prev) => prev.intersection(&set).copied().collect(),
                            None => set,
                        });
                    }
                    QueryClause::Phrase { field, phrase } => {
                        let phrase_lower = phrase.to_lowercase();
                        let words = tokenize_plain(&phrase_lower);
                        if words.is_empty() {
                            continue;
                        }
                        // Narrow candidates via first term posting.
                        let first_key = match field {
                            Some(f) => format!("{f}:{}", words[0]),
                            None => words[0].clone(),
                        };
                        let Some(list) = self.postings.get(&first_key) else {
                            candidates = Some(HashSet::new());
                            break;
                        };
                        let mut set = HashSet::new();
                        for &(doc_id, _) in list {
                            let doc_lower = self.docs[doc_id].text.to_lowercase();
                            if doc_lower.contains(&phrase_lower) {
                                set.insert(doc_id);
                                *group_scores.entry(doc_id).or_insert(0.0) += 2.0;
                            }
                        }
                        candidates = Some(match candidates {
                            Some(prev) => prev.intersection(&set).copied().collect(),
                            None => set,
                        });
                    }
                    QueryClause::NegatedTerm { field, term } => {
                        let key = match field {
                            Some(f) => format!("{f}:{term}"),
                            None => term.clone(),
                        };
                        if let Some(list) = self.postings.get(&key) {
                            let excluded: HashSet<usize> = list.iter().map(|&(d, _)| d).collect();
                            if let Some(ref mut prev) = candidates {
                                prev.retain(|d| !excluded.contains(d));
                            }
                        }
                    }
                }
            }

            if let Some(matched) = candidates {
                for doc_id in matched {
                    let s = group_scores.get(&doc_id).copied().unwrap_or(0.0);
                    *scores.entry(doc_id).or_insert(0.0) += s;
                }
            }
        }

        let mut hits: Vec<FtsHit> = scores
            .into_iter()
            .map(|(doc_id, score)| {
                let d = &self.docs[doc_id];
                FtsHit {
                    doc_id,
                    line_number: d.line_number,
                    score,
                    text: d.text.clone(),
                }
            })
            .collect();

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.line_number.cmp(&b.line_number))
        });
        hits.truncate(limit);
        hits
    }
}

struct ParsedQuery {
    groups: Vec<Vec<QueryClause>>,
}

fn parse_query(raw: &str) -> ParsedQuery {
    let mut groups: Vec<Vec<QueryClause>> = vec![Vec::new()];
    let mut chars = raw.trim().chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        if ch == '"' {
            chars.next();
            let mut phrase = String::new();
            for c in chars.by_ref() {
                if c == '"' {
                    break;
                }
                phrase.push(c);
            }
            if !phrase.trim().is_empty() {
                groups
                    .last_mut()
                    .unwrap()
                    .push(QueryClause::Phrase { field: None, phrase });
            }
            continue;
        }

        let mut token = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                break;
            }
            token.push(c);
            chars.next();
        }

        if token == "OR" {
            if !groups.last().unwrap().is_empty() {
                groups.push(Vec::new());
            }
            continue;
        }
        if token == "AND" {
            continue;
        }

        if let Some(rest) = token.strip_prefix('-') {
            if let Some(clause) = parse_single_token(rest, true) {
                groups.last_mut().unwrap().push(clause);
            }
        } else if let Some(clause) = parse_single_token(&token, false) {
            groups.last_mut().unwrap().push(clause);
        }
    }

    groups.retain(|g| !g.is_empty());
    ParsedQuery { groups }
}

fn parse_single_token(tok: &str, negated: bool) -> Option<QueryClause> {
    if let Some((field, val)) = tok.split_once(':') {
        let f = field.trim().to_ascii_lowercase();
        let v = val.trim_matches('"').to_ascii_lowercase();
        if !f.is_empty() && !v.is_empty() {
            return Some(if negated {
                QueryClause::NegatedTerm {
                    field: Some(f),
                    term: v,
                }
            } else {
                QueryClause::Term {
                    field: Some(f),
                    term: v,
                }
            });
        }
    }
    let clean = tokenize_plain(tok).into_iter().next()?;
    Some(if negated {
        QueryClause::NegatedTerm {
            field: None,
            term: clean,
        }
    } else {
        QueryClause::Term {
            field: None,
            term: clean,
        }
    })
}

fn tokenize_plain(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

fn extract_index_tokens(text: &str) -> Vec<String> {
    let mut out = tokenize_plain(text);
    // If the line is a JSON object, also emit `field:term` tokens for field-scoped queries.
    if text.trim_start().starts_with('{') {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) {
            for (k, v) in map {
                let field = k.to_ascii_lowercase();
                let val_str = match v {
                    Value::String(s) => s,
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => continue,
                };
                for tok in tokenize_plain(&val_str) {
                    out.push(format!("{field}:{tok}"));
                }
            }
        }
    }
    out
}

fn file_stamp(path: &Path) -> Result<FileStamp, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let size = meta.len();
    let mtime_nanos = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 4096];
    let n = file.read(&mut buf).map_err(|e| e.to_string())?;
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in &buf[..n] {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(FileStamp {
        size,
        mtime_nanos,
        head_hash: hash,
    })
}

fn cache_path_for(path: &Path) -> Option<PathBuf> {
    let root = config_dir::config_root()?;
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in canonical.to_string_lossy().as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Some(root.join("fts").join(format!("{hash:016x}.json")))
}

fn try_load_cache(path: &Path, stamp: &FileStamp) -> Option<FtsIndex> {
    let cache_path = cache_path_for(path)?;
    let bytes = fs::read(cache_path).ok()?;
    let index: FtsIndex = serde_json::from_slice(&bytes).ok()?;
    if index.stamp.as_ref() == Some(stamp) {
        Some(index)
    } else {
        None
    }
}

fn try_save_cache(path: &Path, index: &FtsIndex) -> Result<(), String> {
    let Some(cache_path) = cache_path_for(path) else {
        return Ok(());
    };
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec(index).map_err(|e| e.to_string())?;
    fs::write(cache_path, bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bm25_ranks_relevant_lines_and_supports_phrases_and_fields() {
        let lines = vec![
            (1, r#"{"host":"prod-1","msg":"connection refused on port 5432"}"#),
            (2, r#"{"host":"staging","msg":"connection reset by peer"}"#),
            (3, r#"{"host":"prod-2","msg":"startup complete, connection pool ready"}"#),
        ];
        let idx = FtsIndex::from_lines(lines);

        let phrase_hits = idx.search(r#""connection refused""#, 10);
        assert_eq!(phrase_hits.len(), 1);
        assert_eq!(phrase_hits[0].line_number, 1);

        let field_hits = idx.search("host:prod connection", 10);
        assert_eq!(field_hits.len(), 2);

        let neg_hits = idx.search("connection -staging", 10);
        assert!(neg_hits.iter().all(|h| h.line_number != 2));
    }

    #[test]
    fn corrupt_cache_never_fails_open_or_build() {
        config_dir::isolate_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sample.log");
        fs::write(&file, "error connection refused\nok ready\n").unwrap();

        let idx = FtsIndex::open_or_build(&file).unwrap();
        assert_eq!(idx.docs.len(), 2);
        let hits = idx.search("refused", 5);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line_number, 1);

        // Corrupt the cache file and verify open_or_build recovers cleanly.
        if let Some(cache) = cache_path_for(&file) {
            fs::write(cache, b"{corrupt json").unwrap();
        }
        let rebuilt = FtsIndex::open_or_build(&file).unwrap();
        assert_eq!(rebuilt.docs.len(), 2);
    }
}
