//! egui presentation of consent requests. The queue lives in `thoth-core`.

pub use thoth_core::consent::*;

pub mod modal;
pub use modal::{ConsentModal, ConsentModalProps};
