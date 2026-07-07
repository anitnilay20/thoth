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
    ToggleSwitch, Typography, VSplit,
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
    /// A resizable vertical [`VSplit`] (top over bottom, draggable divider).
    VSplit(VSplit),
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
    ///
    /// Note: objects/arrays build a [`JsonTree`] with the default (shared) id.
    /// If several such cells are visible at once, set a unique
    /// [`JsonTree::id`](crate::components::JsonTree::id) per instance so their
    /// expansion state doesn't leak across cells (this helper has no per-cell
    /// context to assign one).
    pub fn json_cell(value: &serde_json::Value) -> Self {
        use serde_json::Value;
        match value {
            Value::Null => RenderNode::Text(
                Typography::builder()
                    .text("null")
                    .italic(true)
                    .color("muted")
                    .build(),
            ),
            Value::Bool(b) => RenderNode::Text(
                Typography::builder()
                    .text(b.to_string())
                    .color("boolean")
                    .build(),
            ),
            Value::Number(n) => RenderNode::Text(
                Typography::builder()
                    .text(n.to_string())
                    .color("number")
                    .build(),
            ),
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

#[cfg(test)]
mod tests {
    use super::RenderNode;
    use crate::components::{
        Button, ButtonColor, Column, Row, Separator, Typography, TypographyVariant,
    };
    use serde_json::{Value, json};

    // ── type tag serialisation ────────────────────────────────────────────────

    #[test]
    fn separator_serialises_with_type_tag() {
        let node = RenderNode::Separator(Separator::plain());
        let v: Value = serde_json::to_value(&node).unwrap();
        assert_eq!(v["type"], "separator");
    }

    #[test]
    fn text_node_serialises_with_type_tag() {
        let node = RenderNode::Text(Typography::builder().text("hello").build());
        let v: Value = serde_json::to_value(&node).unwrap();
        assert_eq!(v["type"], "text");
        assert_eq!(v["text"], "hello");
    }

    #[test]
    fn button_node_serialises_with_type_tag() {
        let node = RenderNode::Button(Button::builder().label("Click me").build());
        let v: Value = serde_json::to_value(&node).unwrap();
        assert_eq!(v["type"], "button");
        assert_eq!(v["label"], "Click me");
    }

    #[test]
    fn column_node_serialises_with_type_tag() {
        let node = RenderNode::Column(Column::builder().gap(8.0).build());
        let v: Value = serde_json::to_value(&node).unwrap();
        assert_eq!(v["type"], "column");
        assert_eq!(v["gap"], 8.0);
    }

    #[test]
    fn row_node_serialises_with_type_tag() {
        let node = RenderNode::Row(Row::builder().padding(4.0).build());
        let v: Value = serde_json::to_value(&node).unwrap();
        assert_eq!(v["type"], "row");
        assert_eq!(v["padding"], 4.0);
    }

    // ── nested children serialisation ─────────────────────────────────────────

    #[test]
    fn column_with_children_serialises_correctly() {
        let node = RenderNode::Column(
            Column::builder()
                .gap(4.0)
                .children(vec![
                    RenderNode::Text(Typography::builder().text("line 1").build()),
                    RenderNode::Text(Typography::builder().text("line 2").build()),
                ])
                .build(),
        );
        let v: Value = serde_json::to_value(&node).unwrap();
        assert_eq!(v["type"], "column");
        assert_eq!(v["children"].as_array().unwrap().len(), 2);
        assert_eq!(v["children"][0]["type"], "text");
        assert_eq!(v["children"][1]["text"], "line 2");

        // Round-trip: the recursive `children: Vec<RenderNode>` must survive
        // deserialization, not just serialize to the right shape.
        let back: RenderNode = serde_json::from_value(v).unwrap();
        let RenderNode::Column(col) = back else {
            panic!("expected RenderNode::Column, got {back:?}");
        };
        assert_eq!(col.children.len(), 2);
        match (&col.children[0], &col.children[1]) {
            (RenderNode::Text(a), RenderNode::Text(b)) => {
                assert_eq!(a.text, "line 1");
                assert_eq!(b.text, "line 2");
            }
            other => panic!("expected two Text children, got {other:?}"),
        }
    }

    // ── deserialisation ───────────────────────────────────────────────────────

    #[test]
    fn separator_deserialises_from_json() {
        let json = r#"{"type":"separator","margin-top":0.0,"margin-bottom":0.0}"#;
        let node: RenderNode = serde_json::from_str(json).unwrap();
        assert!(matches!(node, RenderNode::Separator(_)));
    }

    #[test]
    fn text_node_deserialises_from_json() {
        let json = r#"{"type":"text","text":"hello"}"#;
        let node: RenderNode = serde_json::from_str(json).unwrap();
        if let RenderNode::Text(t) = node {
            assert_eq!(t.text, "hello");
        } else {
            panic!("expected RenderNode::Text");
        }
    }

    #[test]
    fn unknown_type_deserialises_as_unknown_variant() {
        let json = r#"{"type":"future-widget-9000","some-field":true}"#;
        let node: RenderNode = serde_json::from_str(json).unwrap();
        assert!(matches!(node, RenderNode::Unknown));
    }

    #[test]
    fn round_trip_text_node() {
        let original = RenderNode::Text(
            Typography::builder()
                .text("round-trip")
                .variant(TypographyVariant::Caption)
                .build(),
        );
        let json = serde_json::to_string(&original).unwrap();
        let restored: RenderNode = serde_json::from_str(&json).unwrap();
        if let RenderNode::Text(t) = restored {
            assert_eq!(t.text, "round-trip");
            assert_eq!(t.variant, TypographyVariant::Caption);
        } else {
            panic!("expected RenderNode::Text after round-trip");
        }
    }

    #[test]
    fn round_trip_button_node_with_color() {
        let original = RenderNode::Button(
            Button::builder()
                .id("btn1")
                .label("Save")
                .color(ButtonColor::Primary)
                .build(),
        );
        let json = serde_json::to_string(&original).unwrap();
        let restored: RenderNode = serde_json::from_str(&json).unwrap();
        if let RenderNode::Button(b) = restored {
            assert_eq!(b.id, "btn1");
            assert_eq!(b.label, "Save");
            assert_eq!(b.color, ButtonColor::Primary);
        } else {
            panic!("expected RenderNode::Button after round-trip");
        }
    }

    // ── RenderNode::text() constructor ────────────────────────────────────────

    #[test]
    fn text_constructor_produces_body_typography() {
        let node = RenderNode::text("hello");
        if let RenderNode::Text(t) = node {
            assert_eq!(t.text, "hello");
            assert_eq!(t.variant, TypographyVariant::Body);
        } else {
            panic!("expected RenderNode::Text");
        }
    }

    // ── RenderNode::json_cell() ───────────────────────────────────────────────

    #[test]
    fn json_cell_null_produces_italic_muted_text() {
        let node = RenderNode::json_cell(&Value::Null);
        if let RenderNode::Text(t) = node {
            assert_eq!(t.text, "null");
            assert!(t.italic);
            assert_eq!(t.color.as_deref(), Some("muted"));
        } else {
            panic!("expected RenderNode::Text for null");
        }
    }

    #[test]
    fn json_cell_bool_produces_colored_text() {
        let node = RenderNode::json_cell(&Value::Bool(true));
        if let RenderNode::Text(t) = node {
            assert_eq!(t.text, "true");
            assert_eq!(t.color.as_deref(), Some("boolean"));
        } else {
            panic!("expected RenderNode::Text for bool");
        }
    }

    #[test]
    fn json_cell_number_produces_colored_text() {
        let node = RenderNode::json_cell(&json!(42));
        if let RenderNode::Text(t) = node {
            assert_eq!(t.text, "42");
            assert_eq!(t.color.as_deref(), Some("number"));
        } else {
            panic!("expected RenderNode::Text for number");
        }
    }

    #[test]
    fn json_cell_string_produces_plain_text() {
        let node = RenderNode::json_cell(&Value::String("hello".into()));
        if let RenderNode::Text(t) = node {
            assert_eq!(t.text, "hello");
            assert_eq!(t.color, None); // no syntax tint for strings
        } else {
            panic!("expected RenderNode::Text for string");
        }
    }

    #[test]
    fn json_cell_array_produces_json_tree() {
        let node = RenderNode::json_cell(&json!([1, 2, 3]));
        assert!(matches!(node, RenderNode::JsonTree(_)));
    }

    #[test]
    fn json_cell_object_produces_json_tree() {
        let node = RenderNode::json_cell(&json!({"a": 1}));
        assert!(matches!(node, RenderNode::JsonTree(_)));
    }

    #[test]
    fn json_cell_bool_false_produces_boolean_color() {
        let node = RenderNode::json_cell(&Value::Bool(false));
        if let RenderNode::Text(t) = node {
            assert_eq!(t.text, "false");
            assert_eq!(t.color.as_deref(), Some("boolean"));
        } else {
            panic!("expected RenderNode::Text for bool false");
        }
    }
}

/// Wire-format guards for components whose serde renames / enum tags are part of
/// the plugin↔host contract (a rename here is a breaking protocol change).
#[cfg(test)]
mod wire_format_tests {
    use super::RenderNode;
    use crate::components::*;
    use serde_json::json;

    #[test]
    fn row_renames_bg_color_and_max_width() {
        let v = serde_json::to_value(
            Row::builder()
                .bg_color(BgColor::BgPanel)
                .max_width(true)
                .build(),
        )
        .unwrap();
        assert_eq!(v["bg-color"], json!("bg-panel"));
        assert_eq!(v["max-width"], json!(true));
    }

    #[test]
    fn tabs_renames_content_gap() {
        let v = serde_json::to_value(Tabs::builder().id("t").content_gap(0.0).build()).unwrap();
        assert_eq!(v["content-gap"], json!(0.0));
    }

    #[test]
    fn key_value_list_renames_add_label() {
        let v =
            serde_json::to_value(KeyValueList::builder().add_label("Add header").build()).unwrap();
        assert_eq!(v["add-label"], json!("Add header"));
    }

    #[test]
    fn size_serialises_pascal_case() {
        assert_eq!(serde_json::to_value(Size::Medium).unwrap(), json!("Medium"));
        assert_eq!(serde_json::to_value(Size::Small).unwrap(), json!("Small"));
    }

    #[test]
    fn card_icon_file_is_host_only_not_on_the_wire() {
        // `IconFile` is `#[serde(skip)]` (security): the host may construct it in
        // Rust, but it must never cross the plugin→host wire — so it neither
        // serializes nor deserializes.
        assert!(
            serde_json::to_value(CardIcon::IconFile {
                path: "/x.png".into()
            })
            .is_err()
        );
        let from_wire: Result<CardIcon, _> =
            serde_json::from_value(json!({ "icon-file": { "path": "/etc/passwd" } }));
        assert!(from_wire.is_err());
    }

    #[test]
    fn list_item_postfix_progress_bar_is_externally_tagged() {
        let v = serde_json::to_value(ListItemPostfix::Progress(
            crate::components::Progress::builder().value(0.5).build(),
        ))
        .unwrap();
        assert_eq!(v["Progress"]["value"], json!(0.5));
    }

    #[test]
    fn list_item_prefix_icon_carries_glyph() {
        let v = serde_json::to_value(ListItemPrefix::Icon {
            glyph: "\u{e1de}".into(),
            color: Some("muted".into()),
        })
        .unwrap();
        assert_eq!(v["Icon"]["glyph"], json!("\u{e1de}"));
        assert_eq!(v["Icon"]["color"], json!("muted"));
    }

    #[test]
    fn render_node_table_round_trips() {
        let node = RenderNode::Table(
            TableView::builder()
                .headers(vec!["a".to_string(), "b".to_string()])
                .build(),
        );
        let v = serde_json::to_value(&node).unwrap();
        assert_eq!(v["type"], json!("table"));
        let back: RenderNode = serde_json::from_value(v).unwrap();
        assert!(matches!(back, RenderNode::Table(_)));
    }

    #[test]
    fn render_node_scroll_preserves_id_salt() {
        let node = RenderNode::Scroll(
            Scroll::builder()
                .id("results")
                .child(RenderNode::text("x"))
                .build(),
        );
        let v = serde_json::to_value(&node).unwrap();
        assert_eq!(v["type"], json!("scroll"));
        assert_eq!(v["id"], json!("results"));
    }

    #[test]
    fn button_group_serialises_value_and_active() {
        let node = ButtonGroups::builder()
            .items(vec![
                ButtonGroupItem::builder().value("get").label("GET").build(),
            ])
            .active("get")
            .build();
        let v = serde_json::to_value(node).unwrap();
        assert_eq!(v["active"], json!("get"));
        assert_eq!(v["items"][0]["value"], json!("get"));
    }
}
