//! The render-node DSL: the serializable UI tree the host renders.
//!
//! [`RenderNode`] is the owned, `serde`-tagged tree a plugin produces (as JSON)
//! and the host walks to render. Unlike the host's historical `UiNode` — a flat
//! enum that re-declared every widget's fields inline — each **leaf** variant
//! here *wraps the corresponding component struct* (e.g. [`RenderNode::Button`]
//! holds a [`Button`]). That keeps a single source of truth for each widget's
//! data and lets the renderer delegate to the component's own rendering instead
//! of a parallel match that drifts. **Container** variants are recursive and
//! hold `children: Vec<RenderNode>`.
//!
//! The tree is fully owned (`'static`): it deserializes cleanly from the
//! internally-tagged `{"type": "...", ...}` JSON the host already speaks, and it
//! can be cached/retained across frames (the host stores parsed trees in egui
//! memory). Rendering is added separately under the `egui` feature.
//!
//! Two construction paths are intended:
//! - **DSL path** — deserialize a `RenderNode` tree from plugin JSON.
//! - **UI path** — build the tree in Rust from the component builders, e.g.
//!   `RenderNode::Button(Button::builder().label("Save").build())`.

#[cfg(feature = "egui")]
mod render;

use serde::{Deserialize, Serialize};

use crate::components::{
    Breadcrumbs, Button, ButtonGroups, DataRow, IconButton, Input, JsonTree, Select, Separator,
    SidebarHeader, TableView, ToggleSwitch, Typography,
};

/// A node in the Thoth UI tree.
///
/// Serialized with an internal `"type"` tag (kebab-case), so a button is
/// `{"type": "button", "label": "Save", ...}` and a row is
/// `{"type": "row", "children": [...]}`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum RenderNode {
    // ── Containers (recursive) ───────────────────────────────────────────────
    /// Lay children out left-to-right.
    Row {
        /// Child nodes, in order.
        #[serde(default)]
        children: Vec<RenderNode>,
        /// Horizontal gap between children, in points.
        #[serde(default)]
        gap: f32,
    },
    /// Lay children out top-to-bottom.
    Column {
        /// Child nodes, in order.
        #[serde(default)]
        children: Vec<RenderNode>,
        /// Vertical gap between children, in points.
        #[serde(default)]
        gap: f32,
    },
    /// A scrollable region wrapping a single child.
    Scroll {
        /// The scrolled content.
        child: Box<RenderNode>,
        /// Optional fixed max height, in points.
        #[serde(default)]
        max_height: Option<f32>,
    },
    /// Empty space of a fixed size, in points.
    Spacer {
        /// The amount of space.
        size: f32,
    },

    // ── Leaf widgets (wrap component structs) ────────────────────────────────
    /// A [`Button`].
    Button(Button),
    /// A styled text run ([`Typography`]).
    Text(Typography),
    /// An [`IconButton`].
    IconButton(IconButton),
    /// A [`ToggleSwitch`].
    Toggle(ToggleSwitch),
    /// A [`Breadcrumbs`] trail.
    Breadcrumbs(Breadcrumbs),
    /// A segmented [`ButtonGroups`] control.
    ButtonGroup(ButtonGroups),
    /// A [`Separator`] divider.
    Separator(Separator),
    /// A text [`Input`].
    Input(Input),
    /// A [`Select`] dropdown.
    Select(Select),
    /// A single [`DataRow`].
    DataRow(DataRow),
    /// A [`TableView`] grid.
    Table(TableView),
    /// A [`JsonTree`] viewer.
    JsonTree(JsonTree),
    /// A [`SidebarHeader`].
    SidebarHeader(SidebarHeader),

    /// An arbitrary host-drawn widget — the UI-path escape hatch. Never
    /// serialized (the DSL can't express arbitrary code), so it only exists in
    /// trees built in Rust. Construct via [`RenderNode::custom`].
    #[cfg(feature = "egui")]
    #[serde(skip)]
    Custom(CustomWidget),
}

/// A type-erased draw closure carried by [`RenderNode::Custom`].
///
/// Wraps a `FnMut(&mut egui::Ui)` in an `Arc<Mutex<…>>` so [`RenderNode`] stays
/// `Clone + Send + Sync` (and thus retainable in egui memory). It is never
/// serialized.
#[cfg(feature = "egui")]
#[derive(Clone)]
pub struct CustomWidget(std::sync::Arc<std::sync::Mutex<dyn FnMut(&mut egui::Ui) + Send>>);

#[cfg(feature = "egui")]
impl CustomWidget {
    /// Wrap a draw closure.
    pub fn new(draw: impl FnMut(&mut egui::Ui) + Send + 'static) -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(draw)))
    }

    /// Invoke the closure to draw into `ui`.
    pub(crate) fn show(&self, ui: &mut egui::Ui) {
        if let Ok(mut draw) = self.0.lock() {
            draw(ui);
        }
    }
}

#[cfg(feature = "egui")]
impl std::fmt::Debug for CustomWidget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CustomWidget(..)")
    }
}
