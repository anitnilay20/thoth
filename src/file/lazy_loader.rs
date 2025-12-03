// This module has been refactored into separate components.
// This file remains for backward compatibility and re-exports the new module structure.
//
// See the following modules for the actual implementations:
// - loaders/ndjson.rs: NdjsonFile implementation
// - loaders/json_array.rs: JsonArrayFile implementation
// - loaders/single.rs: SingleValueFile implementation
// - loaders/mod.rs: Common types and FileLoader interface

#[allow(unused_imports)]
pub use crate::file::loaders::{FileType, LazyJsonFile, load_file_auto};
