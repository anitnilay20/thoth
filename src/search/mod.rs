//! Search over an open file.
//!
//! The record-scanning engine is parked while search is rebuilt on top of the
//! DuckDB query engine (#147): filtering becomes a structured predicate
//! compiled to a `WHERE` clause (#53) rather than a full scan in Rust.
//!
//! [`results`] stays compiled because the file viewer's highlighting is
//! expressed in its types — that plumbing is what the new filter will feed.

// TODO(#53): rebuild on top of `FileLoader::fetch` with a compiled WHERE
// clause, then re-enable these and `components::search`.
// mod engine;
// pub use engine::{QueryMode, Search};

#[allow(dead_code)]
pub mod jsonpath;
pub mod results;
