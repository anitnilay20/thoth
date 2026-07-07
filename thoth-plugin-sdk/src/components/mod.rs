//! UI components of the Thoth plugin DSL.
//!
//! Each component is a plain, serializable data type constructed through a
//! [`bon`] builder (e.g. [`Button::builder`]). Plugins build a tree of these
//! and serialize it to JSON for the host; with the `egui` feature the host
//! renders the same types via their `egui::Widget` implementations.

mod badge;
mod breadcrumbs;
mod button;
mod button_group;
mod card;
mod checkbox;
mod code;
mod code_editor;
mod data_row;
#[cfg(feature = "egui")]
pub(crate) mod helpers;
mod icon;
mod icon_button;
mod input;
mod json_tree;
mod key_value_list;
mod layout;
mod link;
mod list;
mod markdown;
mod modal;
mod multi_select;
mod number_input;
mod progress;
mod radio;
mod select;
mod separator;
mod size;
mod sidebar_header;
mod slider;
mod spinner;
mod table_view;
mod tabs;
mod toggle_switch;
mod typography;

pub use badge::Badge;
pub use breadcrumbs::Breadcrumbs;
pub use button::{Button, ButtonColor, ButtonSize, ButtonType};
pub use button_group::{ButtonGroupItem, ButtonGroups};
#[cfg(feature = "egui")]
pub use card::CardEvent;
pub use card::{Card, CardAction, CardIcon};
pub use checkbox::Checkbox;
pub use code::Code;
pub use code_editor::{CodeEditor, CodeEditorOutput, CustomSyntax, RunRequest};
#[cfg(feature = "egui")]
pub use data_row::DataRowOutput;
pub use data_row::{DataRow, DataRowIcon, RowHighlights};
pub use icon::Icon;
pub use icon_button::IconButton;
pub use input::Input;
pub use json_tree::JsonTree;
pub use key_value_list::{KeyValueList, KvEntry};
pub use layout::{
    Align, BgColor, Collapsible, Colored, Column, Footer, Group, KeyValue, Row, Scroll, Spacer,
    Split, VSplit,
};
pub use link::Link;
#[cfg(feature = "egui")]
pub use list::ListEvent;
pub use list::{List, ListItem, ListItemAction, ListItemBadge, ListItemPostfix, ListItemPrefix};
pub use markdown::Markdown;
pub use modal::Modal;
pub use multi_select::MultiSelect;
pub use number_input::NumberInput;
pub use progress::Progress;
pub use radio::Radio;
pub use select::{Select, SelectOption, SelectResponse};
pub use size::Size;
pub use separator::Separator;
pub use sidebar_header::{SidebarHeader, SidebarHeaderAction};
pub use slider::Slider;
pub use spinner::Spinner;
pub use table_view::TableView;
pub use tabs::{TabAction, Tabs};
pub use toggle_switch::ToggleSwitch;
pub use typography::{Typography, TypographyVariant};
