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

/// An interaction event emitted by an interactive node while rendering.
///
/// The renderer collects these into a `Vec<UiEvent>` (the sink threaded through
/// [`RenderNode::show`]); the host forwards them to the plugin's event handler.
/// `kind` is the interaction class (`"change"`, `"click"`, `"toggle"`,
/// `"action"`), and `value` is a string payload (new value, clicked index, …).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiEvent {
    /// Id of the widget that emitted the event.
    pub id: String,
    /// Interaction class — e.g. `"change"`, `"click"`, `"toggle"`, `"action"`.
    pub kind: String,
    /// String payload describing what happened.
    pub value: String,
}

use crate::components::{
    Badge, Breadcrumbs, Button, ButtonGroups, Card, Checkbox, Code, CodeEditor, Collapsible,
    Colored, Column, DataRow, Footer, Group, Icon, IconButton, Input, JsonTree, KeyValue,
    KeyValueList, Link, List, Markdown, Modal, MultiSelect, NumberInput, Progress, Radio, Row,
    Scroll, Select, Separator, SidebarHeader, Slider, Spacer, Spinner, Split, TableView, Tabs,
    ToggleSwitch, Typography,
};

/// A node in the Thoth UI tree.
///
/// Serialized with an internal `"type"` tag (kebab-case), so a button is
/// `{"type": "button", "label": "Save", ...}` and a row is
/// `{"type": "row", "children": [...]}`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum RenderNode {
    // ── Containers (recursive, wrap layout component structs) ────────────────
    /// A horizontal [`Row`].
    Row(Row),
    /// A vertical [`Column`].
    Column(Column),
    /// A [`Scroll`] region.
    Scroll(Scroll),
    /// Fixed [`Spacer`] space.
    Spacer(Spacer),
    /// A proportional [`Split`].
    Split(Split),
    /// A [`Group`] (collapsible, open by default).
    Group(Group),
    /// A [`Collapsible`] (closed by default).
    Collapsible(Collapsible),
    /// A bottom-aligned [`Footer`].
    Footer(Footer),
    /// An inline [`KeyValue`] pair.
    KeyValue(KeyValue),
    /// A [`Colored`] subtree.
    Colored(Colored),

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
    /// A colored pill [`Badge`].
    Badge(Badge),
    /// A standalone [`Icon`] glyph.
    Icon(Icon),
    /// A hyperlink ([`Link`]).
    Link(Link),
    /// A [`Progress`] bar.
    Progress(Progress),
    /// A loading [`Spinner`].
    Spinner(Spinner),
    /// A [`Modal`] overlay dialog. Boxed because `Modal` itself holds a
    /// `RenderNode` body (breaks the recursive-size cycle).
    Modal(Box<Modal>),
    /// A [`Checkbox`].
    Checkbox(Checkbox),
    /// A [`Slider`].
    Slider(Slider),
    /// A numeric [`NumberInput`].
    NumberInput(NumberInput),
    /// A [`Radio`] group.
    Radio(Radio),
    /// A [`MultiSelect`] checkbox list.
    MultiSelect(MultiSelect),
    /// An editable [`KeyValueList`].
    KeyValueList(KeyValueList),
    /// A read-only [`Code`] block.
    Code(Code),
    /// A rendered [`Markdown`] block.
    Markdown(Markdown),
    /// An editable [`CodeEditor`].
    CodeEditor(CodeEditor),
    /// A rich [`List`].
    List(List),
    /// A tabbed [`Tabs`] container.
    Tabs(Tabs),
    /// A content [`Card`]. Boxed because `Card` holds an optional `RenderNode`
    /// body (breaks the recursive-size cycle).
    Card(Box<Card>),

    /// An arbitrary host-drawn widget — the UI-path escape hatch. Never
    /// serialized (the DSL can't express arbitrary code), so it only exists in
    /// trees built in Rust. Construct via [`RenderNode::custom`].
    #[cfg(feature = "egui")]
    #[serde(skip)]
    Custom(CustomWidget),

    /// Fallback for an unrecognised `"type"` — produced when deserialising a
    /// node from a newer host (or plugin) that this SDK version doesn't know.
    /// Renders as nothing, so old plugins degrade gracefully instead of failing
    /// to parse a tree containing newer node types.
    #[serde(other)]
    Unknown,
}

impl RenderNode {
    /// A plain `Body` text node.
    pub fn text(value: impl Into<String>) -> Self {
        RenderNode::Text(Typography::builder().text(value).build())
    }

    /// A node that renders a JSON value coloured by its type — matching the
    /// JSON tree's syntax colours. Numbers and booleans get their type colour,
    /// `null` renders italic + muted, objects/arrays become an interactive
    /// [`JsonTree`], and strings use the default foreground colour.
    pub fn json_cell(value: &serde_json::Value) -> Self {
        use serde_json::Value;
        match value {
            Value::Null => RenderNode::Text(
                Typography::builder().text("null").italic(true).color("muted").build(),
            ),
            Value::Bool(b) => {
                RenderNode::Text(Typography::builder().text(b.to_string()).color("boolean").build())
            }
            Value::Number(n) => {
                RenderNode::Text(Typography::builder().text(n.to_string()).color("number").build())
            }
            // Strings use the default foreground colour (no syntax tint).
            Value::String(s) => RenderNode::text(s.clone()),
            Value::Array(_) | Value::Object(_) => {
                RenderNode::JsonTree(JsonTree::builder().value(value.clone()).build())
            }
        }
    }
}

/// The shared, type-erased draw closure inside a [`CustomWidget`].
#[cfg(feature = "egui")]
type DrawFn = std::sync::Arc<std::sync::Mutex<dyn FnMut(&mut egui::Ui) + Send>>;

/// A type-erased draw closure carried by [`RenderNode::Custom`].
///
/// Wraps a `FnMut(&mut egui::Ui)` in an `Arc<Mutex<…>>` so [`RenderNode`] stays
/// `Clone + Send + Sync` (and thus retainable in egui memory). It is never
/// serialized.
#[cfg(feature = "egui")]
#[derive(Clone)]
pub struct CustomWidget(DrawFn);

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
