//! Re-export of the display-independent session state, which now lives in
//! `thoth-core`. Kept so `crate::app::persistent_state::…` paths keep working.

pub use thoth_core::persistent_state::*;
