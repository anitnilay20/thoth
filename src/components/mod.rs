// ── Host-side shared utilities (the reusable widgets now live in the SDK) ──────
pub mod common;

// The component-trait system stays host-side.
pub use common::traits;

// ── App-specific panels and feature components ────────────────────────────────
pub mod bookmarks;
pub mod central_panel;
pub mod data_source_panel;
pub mod drag_and_drop;
pub mod error_modal;
pub mod file_viewer;
pub mod marketplace;
pub mod recent_files;
pub mod search;
pub mod settings_dialog;
pub mod sidebar;
pub mod status_bar;
pub mod toolbar;
pub mod update_consent_modal;
pub mod welcome;
