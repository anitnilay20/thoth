mod engine;

pub use engine::Search;

#[derive(Debug)]
pub enum SearchMessage {
    StartSearch(Search),
    StopSearch,
}


