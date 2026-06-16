#[cfg(feature = "egui")]
pub mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

/// A breadcrumb trail showing the current location within a hierarchy.
///
/// The path is rendered as `/`-separated segments. An empty or absent path
/// represents the root.
///
/// ```
/// use thoth_plugin_sdk::components::Breadcrumbs;
///
/// let crumbs = Breadcrumbs::builder().path("users[42].settings").build();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
pub struct Breadcrumbs<'a> {
    /// Period-separated path of the current location,
    /// e.g. `"users[42].settings"` -> `["users", "[42]", "settings"]`.
    /// `None` or `""` denotes the root.
    path: Option<&'a str>,
    separator: Option<&'a str>,
}
