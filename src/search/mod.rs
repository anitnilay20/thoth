mod engine;
pub mod results;

pub use engine::Search;

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

    pub fn query(&self) -> Option<&str> {
        match self {
            SearchMessage::StartSearch(search) => Some(&search.query),
            SearchMessage::StopSearch => None,
        }
    }

    pub fn create_search(query: String, match_case: bool) -> Option<Self> {
        let search = Search {
            query,
            match_case,
            scanning: true,
            ..Search::default()
        };
        Some(Self::StartSearch(search))
    }
}
