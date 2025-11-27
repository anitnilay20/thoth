use std::ops::Range;
use std::sync::Arc;

/// Collection of search hits plus aggregate statistics.
#[derive(Default, Debug, Clone)]
pub struct SearchResults {
    hits: Vec<SearchHit>,
    stats: SearchStats,
}

impl SearchResults {
    pub fn new(hits: Vec<SearchHit>, total_records: usize) -> Self {
        let matched_records = hits.len();
        Self {
            hits,
            stats: SearchStats {
                total_records,
                matched_records,
            },
        }
    }

    pub fn len(&self) -> usize {
        self.hits.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    pub fn clear(&mut self) {
        self.hits.clear();
        self.stats.matched_records = 0;
        self.stats.total_records = 0;
    }

    pub fn get(&self, idx: usize) -> Option<&SearchHit> {
        self.hits.get(idx)
    }
}

/// Describes a single record that matched the query.
#[derive(Default, Debug, Clone)]
pub struct SearchHit {
    pub record_index: usize,
    #[allow(dead_code)]
    pub fragments: Vec<MatchFragment>,
}

/// Metadata for each matched fragment (highlight span, path, etc.).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MatchFragment {
    pub fragment_id: u32,
    pub target: MatchTarget,
    pub byte_range: Range<u32>,
    pub path: Option<Arc<str>>,
    pub confidence: f32,
}

impl Default for MatchFragment {
    fn default() -> Self {
        Self {
            fragment_id: 0,
            target: MatchTarget::RawRecord,
            byte_range: 0..0,
            path: None,
            confidence: 0.0,
        }
    }
}

/// Where the match occurred (raw blob vs structured field).
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub enum MatchTarget {
    #[default]
    RawRecord,
    JsonField {
        component: FieldComponent,
    },
}

/// Which part of the structured field matched.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default)]
pub enum FieldComponent {
    Key,
    #[default]
    Value,
    EntireRow,
}

/// Aggregate metrics for search execution.
#[derive(Default, Debug, Clone)]
pub struct SearchStats {
    pub total_records: usize,
    pub matched_records: usize,
}
