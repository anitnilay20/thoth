// ── Common reusable primitives ────────────────────────────────────────────────
pub mod common;

// Re-export common modules at the `components::` level so existing import
// paths (e.g. `crate::components::button::Button`) continue to work.
pub use common::breadcrumbs;
pub use common::button;
pub use common::card;
pub use common::data_row;
pub use common::icon_button;
pub use common::input;
pub use common::list;
pub use common::table_view;
pub use common::toggle_switch;
pub use common::traits;
pub use common::typography;

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
