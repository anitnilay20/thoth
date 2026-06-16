//! UI components of the Thoth plugin DSL.
//!
//! Each component is a plain, serializable data type constructed through a
//! [`bon`] builder (e.g. [`Button::builder`]). Plugins build a tree of these
//! and serialize it to JSON for the host; with the `egui` feature the host
//! renders the same types via their `egui::Widget` implementations.

mod breadcrumbs;
mod button;
mod button_group;
mod data_row;
mod icon_button;
mod input;
mod json_tree;
mod select;
mod separator;
mod sidebar_header;
mod table_view;
mod toggle_switch;
mod typography;

pub use breadcrumbs::Breadcrumbs;
pub use button::{Button, ButtonColor, ButtonSize, ButtonType};
pub use button_group::ButtonGroups;
pub use data_row::{DataRow, DataRowIcon, RowHighlights};
#[cfg(feature = "egui")]
pub use data_row::DataRowOutput;
pub use icon_button::IconButton;
pub use input::Input;
pub use json_tree::JsonTree;
pub use select::{Select, SelectOption, SelectSize};
pub use separator::Separator;
pub use sidebar_header::{SidebarHeader, SidebarHeaderAction};
pub use table_view::TableView;
pub use toggle_switch::ToggleSwitch;
pub use typography::{Typography, TypographyVariant};
