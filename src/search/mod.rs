mod engine;
mod jsonpath;
pub mod results;

pub use engine::{QueryMode, Search};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum SearchMessage {
    StartSearch(Search),
    #[allow(dead_code)]
    StopSearch,
}

impl SearchMessage {
    pub fn is_searching(&self) -> bool {
        match self {
            SearchMessage::StartSearch(search) => search.scanning,
            SearchMessage::StopSearch => false,
        }
    }

    pub fn history_entry(&self) -> Option<String> {
        match self {
            SearchMessage::StartSearch(search) => {
                Some(encode_history_entry(&search.query, search.query_mode))
            }
            SearchMessage::StopSearch => None,
        }
    }

    pub fn create_search(query: String, match_case: bool, query_mode: QueryMode) -> Option<Self> {
        let search = Search {
            query,
            match_case,
            query_mode,
            scanning: true,
            ..Search::default()
        };
        Some(Self::StartSearch(search))
    }
}

#[derive(Serialize)]
struct StoredQueryEntry<'a> {
    mode: QueryMode,
    query: &'a str,
}

#[derive(Deserialize)]
struct StoredQueryEntryOwned {
    mode: QueryMode,
    query: String,
}

fn encode_history_entry(query: &str, mode: QueryMode) -> String {
    serde_json::to_string(&StoredQueryEntry { mode, query }).unwrap_or_else(|_| query.to_string())
}

pub fn decode_history_entry(entry: &str) -> (QueryMode, String) {
    serde_json::from_str::<StoredQueryEntryOwned>(entry)
        .map(|parsed| (parsed.mode, parsed.query))
        .unwrap_or_else(|_| (QueryMode::Text, entry.to_string()))
}
