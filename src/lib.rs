//! Thoth host
//!
//! The desktop application. Display-independent logic is re-exported from
//! `thoth-core` (and the command runtime from `thoth-cli`) so existing
//! `thoth::file`, `thoth::settings`, `thoth::cli` paths keep working.

pub use thoth_core::*;
pub use thoth_cli::{cli, headless};

pub mod app;
pub mod components;
pub mod consent;
pub mod notification;
pub mod state;

#[cfg(test)]
mod test_support;
