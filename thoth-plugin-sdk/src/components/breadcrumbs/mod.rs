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
#[derive(Debug, Clone, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Breadcrumbs {
    /// Delimiter-separated path of the current location,
    /// e.g. `"users.42.settings"` -> `["users", "[42]", "settings"]`.
    /// `None` shows "No selection"; `""` denotes the root.
    #[serde(default)]
    pub path: Option<String>,
    /// Path delimiter used to split/join segments. Defaults to `"."`.
    #[serde(default)]
    pub separator: Option<String>,
}
