//! Thoth CLI
//!
//! The display-free command runtime and `clap` front-end. Re-exports the whole
//! of [`thoth_core`] so existing `crate::file`, `crate::plugin`, `crate::settings`
//! paths inside the CLI modules keep resolving.

pub use thoth_core::*;

pub mod cli;
pub mod headless;
